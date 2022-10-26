use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use anyhow::anyhow;
use async_trait::async_trait;
use shuttle_common::{database, LogItem};
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory, Logger, ServiceName,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tracing::{info, instrument, trace};

pub mod args;
pub mod error;

pub struct Legacy {
    so_path: Option<PathBuf>,
    port: Option<u16>,
}

impl Legacy {
    pub fn new() -> Self {
        Self {
            so_path: None,
            port: None,
        }
    }

    pub async fn load(&mut self, so_path: PathBuf) -> Result<bool, error::Error> {
        self.so_path = Some(so_path);

        Ok(true)
    }

    pub async fn start(&mut self) -> Result<(), error::Error> {
        let port = 8000;
        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let mut factory = DummyFactory::new();
        let (logger, _rx) = get_logger();
        let so_path = self
            .so_path
            .as_ref()
            .ok_or_else(|| -> error::Error {
                error::Error::Start(anyhow!("trying to start a service that was not loaded"))
            })?
            .clone();

        let service = load_service(address, so_path, &mut factory, logger)
            .await
            .unwrap();

        self.port = Some(port);

        _ = tokio::spawn(run(service, address)).await;

        Ok(())
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
