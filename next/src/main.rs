use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf, str::FromStr};

use async_trait::async_trait;
use clap::Parser;
use shuttle_common::{database, LogItem};
use shuttle_next::args::Args;
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory, Logger, ServiceName,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tracing::{info, instrument, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    trace!(args = ?args, "parsed args");

    let address: SocketAddr = "127.0.0.1:8000".parse().unwrap();
    let mut factory = DummyFactory::new();
    let (logger, _rx) = get_logger();
    let so_path = PathBuf::from(args.file_path.as_str());

    let service = load_service(address, so_path, &mut factory, logger)
        .await
        .unwrap();

    _ = tokio::spawn(run(service, address)).await;
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
) -> shuttle_next::error::Result<LoadedService> {
    let loader = Loader::from_so_file(so_path)?;

    Ok(loader.load(factory, addr, logger).await?)
}

struct DummyFactory {
    service_name: ServiceName,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            service_name: ServiceName::from_str("next").unwrap(),
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
