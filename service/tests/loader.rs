use std::{
    net::{Ipv4Addr, SocketAddr},
    process::Command,
};

mod helpers;

use async_trait::async_trait;
use helpers::PostgresInstance;
use shuttle_service::{
    loader::{Loader, LoaderError},
    Error, Factory,
};

struct DummyFactory {
    postgres_instance: Option<PostgresInstance>,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            postgres_instance: None,
        }
    }

    fn new_with_postgres() -> Self {
        Self {
            postgres_instance: Some(PostgresInstance::new()),
        }
    }
}

#[async_trait]
impl Factory for DummyFactory {
    async fn get_sql_connection_string(&mut self) -> Result<String, Error> {
        let uri = if let Some(postgres_instance) = &self.postgres_instance {
            postgres_instance.get_uri()
        } else {
            let postgres_instance = PostgresInstance::new();
            let uri = postgres_instance.get_uri();
            self.postgres_instance = Some(postgres_instance);
            uri
        };

        Ok(uri)
    }
}

#[test]
fn not_shuttle() {
    Command::new("cargo")
        .args(["build", "--release"])
        .current_dir("tests/resources/not-shuttle")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let result =
        Loader::from_so_file("tests/resources/not-shuttle/target/release/libnot_shuttle.so");

    assert!(matches!(result, Err(LoaderError::GetEntrypoint(_))));
}

#[tokio::test]
async fn sleep_async() {
    Command::new("cargo")
        .args(["build", "--release"])
        .current_dir("tests/resources/sleep-async")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let loader =
        Loader::from_so_file("tests/resources/sleep-async/target/release/libsleep_async.so")
            .unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    // let Loader { thread_handle, .. } = loader.load(&mut factory, addr).unwrap();

    // thread_handle.unwrap().await.unwrap().unwrap();
    let Loader { service, .. } = loader.load(&mut factory, addr).unwrap();

    service.lock().unwrap().shutdown();
}

#[tokio::test]
async fn sqlx_pool() {
    Command::new("cargo")
        .args(["build", "--release"])
        .current_dir("tests/resources/sqlx-pool")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let loader =
        Loader::from_so_file("tests/resources/sqlx-pool/target/release/libsqlx_pool.so").unwrap();

    // Initialise a Factory with a pre-existing PostgresInstance.
    // There is a need to wait for the instance to be reachable through the assigned port, which requires
    // asynchronous code. This must happen in this tokio::Runtime and not in the inner one.
    let mut factory = DummyFactory::new_with_postgres();
    let instance = factory.postgres_instance.as_ref().unwrap();
    instance.wait_for_ready();
    instance.wait_for_connectable().await;

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let Loader { service, .. } = loader.load(&mut factory, addr).unwrap();

    service.lock().unwrap().shutdown();
}
