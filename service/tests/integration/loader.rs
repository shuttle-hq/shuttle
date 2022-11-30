use crate::helpers::{loader::build_so_create_loader, sqlx::PostgresInstance};

use shuttle_common::log::Level;
use shuttle_common::LogItem;
use shuttle_service::loader::LoaderError;
use shuttle_service::{database, Error, Factory, Logger, ServiceName};
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::exit;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use async_trait::async_trait;

const RESOURCES_PATH: &str = "tests/resources";

struct DummyFactory {
    postgres_instance: Option<PostgresInstance>,
    service_name: ServiceName,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            postgres_instance: None,
            service_name: ServiceName::from_str("test").unwrap(),
        }
    }
}

fn get_logger() -> (Logger, UnboundedReceiver<LogItem>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let logger = Logger::new(tx, Default::default());

    (logger, rx)
}

#[async_trait]
impl Factory for DummyFactory {
    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }

    async fn get_db_connection_string(&mut self, _: database::Type) -> Result<String, Error> {
        let uri = if let Some(postgres_instance) = &self.postgres_instance {
            postgres_instance.get_uri()
        } else {
            let postgres_instance = PostgresInstance::new();
            postgres_instance.wait_for_ready();
            postgres_instance.wait_for_connectable().await;
            let uri = postgres_instance.get_uri();
            self.postgres_instance = Some(postgres_instance);
            uri
        };

        Ok(uri)
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, Error> {
        panic!("did not expect any loader test to get secrets")
    }

    fn get_build_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
        panic!("did not expect any loader test to get the build path")
    }

    fn get_storage_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
        panic!("did not expect any loader test to get the storage path")
    }
}

#[test]
fn not_shuttle() {
    let result = build_so_create_loader(RESOURCES_PATH, "not-shuttle");
    assert!(matches!(result, Err(LoaderError::GetEntrypoint(_))));
}

#[tokio::test]
async fn sleep_async() {
    let loader = build_so_create_loader(RESOURCES_PATH, "sleep-async").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (logger, _rx) = get_logger();
    let (handler, _) = loader.load(&mut factory, addr, logger).await.unwrap();

    // Give service some time to start up
    tokio::time::sleep(Duration::from_secs(1)).await;

    tokio::spawn(async {
        // Time is less than sleep in service
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("Test failed as async service was not aborted");
        exit(1);
    });

    handler.abort();
    assert!(handler.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn sleep() {
    let loader = build_so_create_loader(RESOURCES_PATH, "sleep").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (logger, _rx) = get_logger();
    let (handler, _) = loader.load(&mut factory, addr, logger).await.unwrap();

    // Give service some time to start up
    tokio::time::sleep(Duration::from_secs(1)).await;

    tokio::spawn(async {
        // Time is less than sleep in service
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("Test failed as blocking service was not aborted");
        exit(1);
    });

    handler.abort();
    assert!(handler.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn sqlx_pool() {
    let loader = build_so_create_loader(RESOURCES_PATH, "sqlx-pool").unwrap();

    // Make sure we'll get a log entry
    std::env::set_var("RUST_LOG", "info");

    // Don't initialize a pre-existing PostgresInstance here because the `PostgresInstance::wait_for_connectable()`
    // code has `awaits` and we want to make sure they do not block inside `Service::build()`.
    // At the same time we also want to test the PgPool is created on the correct runtime (ie does not cause a
    // "has to run on a tokio runtime" error)
    let mut factory = DummyFactory::new();

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (logger, mut rx) = get_logger();
    let (handler, _) = loader.load(&mut factory, addr, logger).await.unwrap();

    handler.await.unwrap().unwrap();

    let log = rx.recv().await.unwrap();
    let value = serde_json::from_slice::<serde_json::Value>(&log.fields).unwrap();
    let message = value
        .as_object()
        .unwrap()
        .get("message")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        message.starts_with("SELECT 'Hello world';"),
        "got: {}",
        message
    );
    assert_eq!(log.target, "sqlx::query");
    assert_eq!(log.level, Level::Info);
}

#[tokio::test]
async fn build_panic() {
    let loader = build_so_create_loader(RESOURCES_PATH, "build-panic").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (logger, _rx) = get_logger();

    if let Err(Error::BuildPanic(msg)) = loader.load(&mut factory, addr, logger).await {
        assert_eq!(&msg, "panic in build");
    } else {
        panic!("expected `Err(Error::BuildPanic(_))`");
    }
}

#[tokio::test]
async fn bind_panic() {
    let loader = build_so_create_loader(RESOURCES_PATH, "bind-panic").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (logger, _rx) = get_logger();

    let (handle, _) = loader.load(&mut factory, addr, logger).await.unwrap();

    if let Err(Error::BindPanic(msg)) = handle.await.unwrap() {
        assert_eq!(&msg, "panic in bind");
    } else {
        panic!("expected `Err(Error::BindPanic(_))`");
    }
}
