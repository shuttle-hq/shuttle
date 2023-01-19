use std::{
    collections::BTreeMap,
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    ops::DerefMut,
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
};

use anyhow::anyhow;
use async_trait::async_trait;
use shuttle_common::{deployment::State, storage_manager::StorageManager, LogItem};
use shuttle_proto::{
    provisioner::provisioner_client::ProvisionerClient,
    runtime::{
        self, runtime_server::Runtime, LoadRequest, LoadResponse, StartRequest, StartResponse,
        StopReason, StopRequest, StopResponse, SubscribeLogsRequest, SubscribeStopRequest,
        SubscribeStopResponse,
    },
};
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory, Logger, ServiceName,
};
use tokio::sync::{broadcast, oneshot};
use tokio::sync::{
    broadcast::Sender,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Endpoint, Request, Response, Status};
use tracing::{error, instrument, trace};
use uuid::Uuid;

use crate::provisioner_factory::{AbstractFactory, AbstractProvisionerFactory};

mod error;

pub struct Legacy<S>
where
    S: StorageManager,
{
    // Mutexes are for interior mutability
    so_path: Mutex<Option<PathBuf>>,
    logs_rx: Mutex<Option<UnboundedReceiver<LogItem>>>,
    logs_tx: Mutex<UnboundedSender<LogItem>>,
    stopped_tx: Sender<(Uuid, StopReason, String)>,
    provisioner_address: Endpoint,
    kill_tx: Mutex<Option<oneshot::Sender<String>>>,
    secrets: Mutex<Option<BTreeMap<String, String>>>,
    storage_manager: S,
}

impl<S> Legacy<S>
where
    S: StorageManager,
{
    pub fn new(provisioner_address: Endpoint, storage_manager: S) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (stopped_tx, _stopped_rx) = broadcast::channel(10);

        Self {
            so_path: Mutex::new(None),
            logs_rx: Mutex::new(Some(rx)),
            logs_tx: Mutex::new(tx),
            stopped_tx,
            kill_tx: Mutex::new(None),
            provisioner_address,
            secrets: Mutex::new(None),
            storage_manager,
        }
    }
}

#[async_trait]
impl<S> Runtime for Legacy<S>
where
    S: StorageManager + 'static,
{
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let LoadRequest { path, secrets, .. } = request.into_inner();
        trace!(path, "loading");

        let so_path = PathBuf::from(path);

        if !so_path.exists() {
            return Err(Status::not_found("'.so' to load does not exist"));
        }

        *self.so_path.lock().unwrap() = Some(so_path);

        *self.secrets.lock().unwrap() = Some(BTreeMap::from_iter(secrets.into_iter()));

        let message = LoadResponse { success: true };
        Ok(Response::new(message))
    }

    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        trace!("legacy starting");

        let provisioner_client = ProvisionerClient::connect(self.provisioner_address.clone())
            .await
            .expect("failed to connect to provisioner");
        let abstract_factory = AbstractProvisionerFactory::new(provisioner_client);

        let so_path = self
            .so_path
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| -> error::Error {
                error::Error::Start(anyhow!("trying to start a service that was not loaded"))
            })
            .map_err(|err| Status::from_error(Box::new(err)))?
            .clone();
        let secrets = self
            .secrets
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| -> error::Error {
                error::Error::Start(anyhow!(
                    "trying to get secrets from a service that was not loaded"
                ))
            })
            .map_err(|err| Status::from_error(Box::new(err)))?
            .clone();

        trace!("prepare done");

        let StartRequest {
            deployment_id,
            service_name,
            port,
        } = request.into_inner();
        let service_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port as u16);

        let service_name = ServiceName::from_str(service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let deployment_id = Uuid::from_slice(&deployment_id).unwrap();

        let mut factory = abstract_factory.get_factory(
            service_name,
            deployment_id,
            secrets,
            self.storage_manager.clone(),
        );
        trace!("got factory");

        let logs_tx = self.logs_tx.lock().unwrap().clone();

        let logger = Logger::new(logs_tx, deployment_id);

        trace!(%service_address, "starting");
        let service = load_service(service_address, so_path, &mut factory, logger)
            .await
            .map_err(|error| match error {
                error::Error::Run(error) => Status::invalid_argument(error.to_string()),
                error => Status::internal(error.to_string()),
            })?;

        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();

        *self.kill_tx.lock().unwrap() = Some(kill_tx);

        // start service as a background task with a kill receiver
        tokio::spawn(run_until_stopped(
            service,
            service_address,
            self.stopped_tx.clone(),
            kill_rx,
            deployment_id,
        ));

        let message = StartResponse { success: true };

        Ok(Response::new(message))
    }

    type SubscribeLogsStream = ReceiverStream<Result<runtime::LogItem, Status>>;

    async fn subscribe_logs(
        &self,
        _request: Request<SubscribeLogsRequest>,
    ) -> Result<Response<Self::SubscribeLogsStream>, Status> {
        let logs_rx = self.logs_rx.lock().unwrap().deref_mut().take();

        if let Some(mut logs_rx) = logs_rx {
            let (tx, rx) = mpsc::channel(1);

            // Move logger items into stream to be returned
            tokio::spawn(async move {
                while let Some(log) = logs_rx.recv().await {
                    tx.send(Ok(log.into())).await.unwrap();
                }
            });

            Ok(Response::new(ReceiverStream::new(rx)))
        } else {
            Err(Status::internal("logs have already been subscribed to"))
        }
    }

    async fn stop(&self, request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        let StopRequest { deployment_id } = request.into_inner();
        let deployment_id = Uuid::from_slice(&deployment_id).unwrap();

        let kill_tx = self.kill_tx.lock().unwrap().deref_mut().take();

        if let Some(kill_tx) = kill_tx {
            trace!(%deployment_id, "stopping deployment");
            if kill_tx
                .send(format!("stopping deployment: {}", &deployment_id))
                .is_err()
            {
                error!("the receiver dropped");
                return Err(Status::internal("failed to stop deployment"));
            }

            Ok(Response::new(StopResponse { success: true }))
        } else {
            Err(Status::internal("failed to stop deployment"))
        }
    }

    type SubscribeStopStream = ReceiverStream<Result<SubscribeStopResponse, Status>>;

    async fn subscribe_stop(
        &self,
        request: Request<SubscribeStopRequest>,
    ) -> Result<Response<Self::SubscribeStopStream>, Status> {
        let SubscribeStopRequest { deployment_id } = request.into_inner();
        let deployment_id = Uuid::from_slice(&deployment_id).unwrap();
        let mut stopped_rx = self.stopped_tx.subscribe();
        let (tx, rx) = mpsc::channel(1);

        // Move the stop channel into a stream to be returned
        tokio::spawn(async move {
            while let Ok((uuid, reason, message)) = stopped_rx.recv().await {
                if uuid == deployment_id {
                    tx.send(Ok(SubscribeStopResponse {
                        reason: reason as i32,
                        message,
                    }))
                    .await
                    .unwrap();
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Run the service until a stop signal is received
#[instrument(skip(service, stopped_tx, kill_rx))]
async fn run_until_stopped(
    service: LoadedService,
    addr: SocketAddr,
    stopped_tx: tokio::sync::broadcast::Sender<(Uuid, StopReason, String)>,
    kill_rx: tokio::sync::oneshot::Receiver<String>,
    deployment_id: Uuid,
) {
    let (handle, library) = service;

    trace!("starting deployment on {}", &addr);
    tokio::select! {
        res = handle => {
            match res.unwrap() {
                Ok(_) => {
                    stopped_tx.send((deployment_id, StopReason::End, String::new())).unwrap();
                }
                Err(error) => {
                    stopped_tx.send((deployment_id, StopReason::Crash, error.to_string())).unwrap();
                }
            }
        },
        message = kill_rx => {
            match message {
                Ok(_) => {
                    stopped_tx.send((deployment_id, StopReason::Request, String::new())).unwrap();
                }
                Err(_) => trace!("the sender dropped")
            };
        }
    }

    tokio::spawn(async move {
        trace!("closing .so file");
        library.close().unwrap();
    });
}

#[instrument(skip(addr, so_path, factory, logger))]
async fn load_service(
    addr: SocketAddr,
    so_path: PathBuf,
    factory: &mut dyn Factory,
    logger: Logger,
) -> error::Result<LoadedService> {
    let loader = Loader::from_so_file(so_path)?;

    Ok(loader.load(factory, addr, logger).await?)
}
