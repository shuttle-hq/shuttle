use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
};

use anyhow::anyhow;
use async_trait::async_trait;
use shuttle_common::{database, LogItem};
use shuttle_runtime_proto::runtime::{
    runtime_server::Runtime, LoadRequest, LoadResponse, StartRequest, StartResponse,
};
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory, Logger, ServiceName,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tonic::{Request, Response, Status};
use tracing::{info, instrument, trace};

mod error;

pub struct Legacy {
    // Mutexes are for interior mutability
    so_path: Mutex<Option<PathBuf>>,
    port: Mutex<Option<u16>>,
}

impl Legacy {
    pub fn new() -> Self {
        Self {
            so_path: Mutex::new(None),
            port: Mutex::new(None),
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
        _request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        let port = 8001;
        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let mut factory = DummyFactory::new();
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

        trace!(%address, "starting");
        let service = load_service(address, so_path, &mut factory, logger)
            .await
            .unwrap();

        _ = tokio::spawn(run(service, address));

        *self.port.lock().unwrap() = Some(port);

        let message = StartResponse {
            success: true,
            port: Some(port as u32),
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

struct DummyFactory {
    service_name: ServiceName,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            service_name: ServiceName::from_str("legacy").unwrap(),
        }
    }
}

#[async_trait]
impl Factory for DummyFactory {
    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }

    async fn get_db_connection_string(
        &mut self,
        _: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        todo!()
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        todo!()
    }
}

fn get_logger() -> (Logger, UnboundedReceiver<LogItem>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let logger = Logger::new(tx, Default::default());

    (logger, rx)
}
