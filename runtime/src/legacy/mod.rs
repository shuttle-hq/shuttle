use std::{
    net::{Ipv4Addr, SocketAddr},
    ops::DerefMut,
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
};

use anyhow::anyhow;
use async_trait::async_trait;
use shuttle_common::LogItem;
use shuttle_proto::{
    provisioner::provisioner_client::ProvisionerClient,
    runtime::{
        self, runtime_server::Runtime, LoadRequest, LoadResponse, StartRequest, StartResponse,
        SubscribeLogsRequest,
    },
};
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory, Logger, ServiceName,
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Endpoint, Request, Response, Status};
use tracing::{info, instrument, trace};
use uuid::Uuid;

use crate::provisioner_factory::{AbstractFactory, AbstractProvisionerFactory};

mod error;

pub struct Legacy {
    // Mutexes are for interior mutability
    so_path: Mutex<Option<PathBuf>>,
    port: Mutex<Option<u16>>,
    logs_rx: Mutex<Option<UnboundedReceiver<LogItem>>>,
    logs_tx: Mutex<UnboundedSender<LogItem>>,
    provisioner_address: Endpoint,
}

impl Legacy {
    pub fn new(provisioner_address: Endpoint) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            so_path: Mutex::new(None),
            port: Mutex::new(None),
            logs_rx: Mutex::new(Some(rx)),
            logs_tx: Mutex::new(tx),
            provisioner_address,
        }
    }
}

#[async_trait]
impl Runtime for Legacy {
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let so_path = request.into_inner().path;
        trace!(so_path, "loading");

        let so_path = PathBuf::from(so_path);
        *self.so_path.lock().unwrap() = Some(so_path);

        let message = LoadResponse { success: true };
        Ok(Response::new(message))
    }

    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        let service_port = 7001;
        let service_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), service_port);

        let request = request.into_inner();

        let provisioner_client = ProvisionerClient::connect(self.provisioner_address.clone())
            .await
            .expect("failed to connect to provisioner");
        let abstract_factory = AbstractProvisionerFactory::new(provisioner_client);

        let service_name = ServiceName::from_str(request.service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let mut factory = abstract_factory.get_factory(service_name);

        let logs_tx = self.logs_tx.lock().unwrap().clone();
        let deployment_id = Uuid::from_slice(&request.deployment_id).unwrap();
        let logger = Logger::new(logs_tx, deployment_id);

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

        trace!(%service_address, "starting");
        let service = load_service(service_address, so_path, &mut factory, logger)
            .await
            .unwrap();

        _ = tokio::spawn(run(service, service_address));

        *self.port.lock().unwrap() = Some(service_port);

        let message = StartResponse {
            success: true,
            port: service_port as u32,
        };

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
}

#[instrument(skip(service))]
async fn run(service: LoadedService, addr: SocketAddr) {
    let (handle, library) = service;

    info!("starting deployment on {}", addr);
    handle.await.unwrap().unwrap();

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
