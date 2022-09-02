use crate::helpers::{loader::build_so_create_loader, sqlx::PostgresInstance};

use shuttle_common::project::ProjectName;
use shuttle_service::loader::LoaderError;
use shuttle_service::{database, Error, Factory};
use std::str::FromStr;

use std::net::{Ipv4Addr, SocketAddr};
use std::process::exit;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use uuid::Uuid;

const RESOURCES_PATH: &str = "tests/resources";

struct DummyFactory {
    postgres_instance: Option<PostgresInstance>,
    project_name: ProjectName,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            postgres_instance: None,
            project_name: ProjectName::from_str("test").unwrap(),
        }
    }
}

#[async_trait]
impl Factory for DummyFactory {
    fn get_project_name(&self) -> ProjectName {
        self.project_name.clone()
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
    let deployment_id = Uuid::new_v4();
    let (tx, _rx) = mpsc::unbounded_channel();
    let (handler, _) = loader
        .load(&mut factory, addr, tx, deployment_id)
        .await
        .unwrap();

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
    let deployment_id = Uuid::new_v4();
    let (tx, _rx) = mpsc::unbounded_channel();
    let (handler, _) = loader
        .load(&mut factory, addr, tx, deployment_id)
        .await
        .unwrap();

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
    let deployment_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (handler, _) = loader
        .load(&mut factory, addr, tx, deployment_id)
        .await
        .unwrap();

    handler.await.unwrap().unwrap();

    let log = rx.recv().await.unwrap();
    assert_eq!(log.deployment_id, deployment_id);
    assert!(
        log.item.body.starts_with("SELECT 'Hello world';"),
        "got: {}",
        log.item.body
    );
    assert_eq!(log.item.target, "sqlx::query");
    assert_eq!(log.item.level, log::Level::Info);
}

#[tokio::test]
async fn build_panic() {
    let loader = build_so_create_loader(RESOURCES_PATH, "build-panic").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let deployment_id = Uuid::new_v4();
    let (tx, _) = mpsc::unbounded_channel();

    if let Err(Error::BuildPanic(msg)) = loader.load(&mut factory, addr, tx, deployment_id).await {
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
    let deployment_id = Uuid::new_v4();
    let (tx, _) = mpsc::unbounded_channel();

    let (handle, _) = loader
        .load(&mut factory, addr, tx, deployment_id)
        .await
        .unwrap();

    if let Err(Error::BindPanic(msg)) = handle.await.unwrap() {
        assert_eq!(&msg, "panic in bind");
    } else {
        panic!("expected `Err(Error::BindPanic(_))`");
    }
}
