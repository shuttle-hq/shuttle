use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
};

use anyhow::anyhow;
use async_trait::async_trait;
use shuttle_common::LogItem;
use shuttle_proto::{
    provisioner::provisioner_client::ProvisionerClient,
    runtime::{runtime_server::Runtime, LoadRequest, LoadResponse, StartRequest, StartResponse},
};
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory, Logger, ServiceName,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tonic::{transport::Endpoint, Request, Response, Status};
use tracing::{info, instrument, trace};

use crate::provisioner_factory::{AbstractFactory, AbstractProvisionerFactory};

mod error;

pub struct Legacy {
    // Mutexes are for interior mutability
    so_path: Mutex<Option<PathBuf>>,
    port: Mutex<Option<u16>>,
    provisioner_address: Endpoint,
}

impl Legacy {
    pub fn new(provisioner_address: Endpoint) -> Self {
        Self {
            so_path: Mutex::new(None),
            port: Mutex::new(None),
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
        let service_port = 8001;
        let service_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), service_port);

        let provisioner_client = ProvisionerClient::connect(self.provisioner_address.clone())
            .await
            .expect("failed to connect to provisioner");
        let abstract_factory = AbstractProvisionerFactory::new(provisioner_client);

        let service_name = ServiceName::from_str(request.into_inner().service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let mut factory = abstract_factory.get_factory(service_name);

        let (logger, _rx) = get_logger();

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

fn get_logger() -> (Logger, UnboundedReceiver<LogItem>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let logger = Logger::new(tx, Default::default());

    (logger, rx)
}
