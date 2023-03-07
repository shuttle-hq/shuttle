use std::{
    collections::BTreeMap,
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    ops::DerefMut,
    str::FromStr,
    sync::Mutex,
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use clap::Parser;
use core::future::Future;
use shuttle_common::{
    storage_manager::{StorageManager, WorkingDirStorageManager},
    LogItem,
};
use shuttle_proto::{
    provisioner::provisioner_client::ProvisionerClient,
    runtime::{
        self,
        runtime_server::{Runtime, RuntimeServer},
        LoadRequest, LoadResponse, StartRequest, StartResponse, StopRequest, StopResponse,
        SubscribeLogsRequest,
    },
};
use shuttle_service::{Factory, Service, ServiceName};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    transport::{Endpoint, Server},
    Request, Response, Status,
};
use tracing::{error, instrument, trace};
use uuid::Uuid;

use crate::provisioner_factory::ProvisionerFactory;

use self::args::Args;

mod args;

pub async fn start(
    loader: impl Loader<ProvisionerFactory<WorkingDirStorageManager>> + Send + 'static,
) {
    let args = Args::parse();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let provisioner_address = args.provisioner_address;
    let mut server_builder =
        Server::builder().http2_keepalive_interval(Some(Duration::from_secs(60)));

    let router = {
        let legacy = Legacy::new(
            provisioner_address,
            loader,
            WorkingDirStorageManager::new(args.storage_manager_path),
        );

        let svc = RuntimeServer::new(legacy);
        server_builder.add_service(svc)
    };

    router.serve(addr).await.unwrap();
}

pub struct Legacy<L, M, S> {
    // Mutexes are for interior mutability
    logs_rx: Mutex<Option<UnboundedReceiver<LogItem>>>,
    logs_tx: UnboundedSender<LogItem>,
    provisioner_address: Endpoint,
    kill_tx: Mutex<Option<oneshot::Sender<String>>>,
    storage_manager: M,
    loader: Mutex<Option<L>>,
    service: Mutex<Option<S>>,
}

impl<L, M, S> Legacy<L, M, S> {
    pub fn new(provisioner_address: Endpoint, loader: L, storage_manager: M) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            logs_rx: Mutex::new(Some(rx)),
            logs_tx: tx,
            kill_tx: Mutex::new(None),
            provisioner_address,
            storage_manager,
            loader: Mutex::new(Some(loader)),
            service: Mutex::new(None),
        }
    }
}

#[async_trait]
pub trait Loader<Fac>
where
    Fac: Factory,
{
    type Service: Service;

    async fn load(
        self,
        factory: Fac,
        logger: shuttle_service::Logger,
    ) -> Result<Self::Service, shuttle_service::Error>;
}

#[async_trait]
impl<F, O, Fac, S> Loader<Fac> for F
where
    F: FnOnce(Fac, shuttle_service::Logger) -> O + Send,
    O: Future<Output = Result<S, shuttle_service::Error>> + Send,
    Fac: Factory + 'static,
    S: Service,
{
    type Service = S;

    async fn load(
        self,
        factory: Fac,
        logger: shuttle_service::Logger,
    ) -> Result<Self::Service, shuttle_service::Error> {
        (self)(factory, logger).await
    }
}

#[async_trait]
impl<L, M, S> Runtime for Legacy<L, M, S>
where
    M: StorageManager + Send + Sync + 'static,
    L: Loader<ProvisionerFactory<M>, Service = S> + Send + 'static,
    S: Service + Send + 'static,
{
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let LoadRequest {
            path,
            secrets,
            service_name,
        } = request.into_inner();
        trace!(path, "loading");

        let secrets = BTreeMap::from_iter(secrets.into_iter());

        let provisioner_client = ProvisionerClient::connect(self.provisioner_address.clone())
            .await
            .context("failed to connect to provisioner")
            .map_err(|err| Status::internal(err.to_string()))?;

        let service_name = ServiceName::from_str(service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let deployment_id = Uuid::new_v4();

        let factory = ProvisionerFactory::new(
            provisioner_client,
            service_name,
            deployment_id,
            secrets,
            self.storage_manager.clone(),
        );
        trace!("got factory");

        let logs_tx = self.logs_tx.clone();
        let logger = shuttle_service::Logger::new(logs_tx, deployment_id);

        let loader = self.loader.lock().unwrap().deref_mut().take().unwrap();

        let service = loader.load(factory, logger).await.unwrap();

        *self.service.lock().unwrap() = Some(service);

        let message = LoadResponse { success: true };
        Ok(Response::new(message))
    }

    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        trace!("legacy starting");
        let service = self.service.lock().unwrap().deref_mut().take();
        let service = service.unwrap();

        let StartRequest { ip, .. } = request.into_inner();
        let service_address = SocketAddr::from_str(&ip)
            .context("invalid socket address")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        let _logs_tx = self.logs_tx.clone();

        trace!(%service_address, "starting");

        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        *self.kill_tx.lock().unwrap() = Some(kill_tx);

        // start service as a background task with a kill receiver
        tokio::spawn(run_until_stopped(service, service_address, kill_rx));

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
                    tx.send(Ok(log.into())).await.expect("to send log");
                }
            });

            Ok(Response::new(ReceiverStream::new(rx)))
        } else {
            Err(Status::internal("logs have already been subscribed to"))
        }
    }

    async fn stop(&self, _request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        let kill_tx = self.kill_tx.lock().unwrap().deref_mut().take();

        if let Some(kill_tx) = kill_tx {
            if kill_tx.send(format!("stopping deployment")).is_err() {
                error!("the receiver dropped");
                return Err(Status::internal("failed to stop deployment"));
            }

            Ok(Response::new(StopResponse { success: true }))
        } else {
            Err(Status::internal("failed to stop deployment"))
        }
    }
}

/// Run the service until a stop signal is received
#[instrument(skip(service, kill_rx))]
async fn run_until_stopped(
    // service: LoadedService,
    service: impl Service,
    addr: SocketAddr,
    kill_rx: tokio::sync::oneshot::Receiver<String>,
) {
    trace!("starting deployment on {}", &addr);
    tokio::select! {
        _ = service.bind(addr) => {
            trace!("deployment stopped on {}", &addr);
        },
        message = kill_rx => {
            match message {
                Ok(msg) => trace!("{msg}"),
                Err(_) => trace!("the sender dropped")
            }
        }
    }
}
