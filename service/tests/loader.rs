use std::{
    net::{Ipv4Addr, SocketAddr},
    process::Command,
};

mod helpers;

use async_trait::async_trait;
use helpers::PostgresInstance;
use shuttle_service::{loader::Loader, Error, Factory};

struct DummyFactory {
    postgres_instance: Option<PostgresInstance>,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            postgres_instance: None,
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
        Loader::from_so_file("tests/resources/sleep-async/target/debug/libsleep_async.so").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (handler, _) = loader.load(&mut factory, addr).unwrap();

    handler.await.unwrap().unwrap();
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
        Loader::from_so_file("tests/resources/sqlx-pool/target/debug/libsqlx_pool.so").unwrap();

    let mut factory = DummyFactory::new();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let (handler, _) = loader.load(&mut factory, addr).unwrap();

    handler.await.unwrap().unwrap();
}
