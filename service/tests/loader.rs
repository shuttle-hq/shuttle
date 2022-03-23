use std::{
    net::{Ipv4Addr, SocketAddr},
    process::Command,
};

use async_trait::async_trait;
use shuttle_service::{loader::Loader, Error, Factory};

struct DummyFactory {}

#[async_trait]
impl Factory for DummyFactory {
    async fn get_sql_connection_string(&mut self) -> Result<String, Error> {
        todo!()
    }
}

#[tokio::test]
async fn hello_world() {
    Command::new("cargo")
        .args(["build"])
        .current_dir("tests/resources/sleep-async")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let loader =
        Loader::from_so_file("tests/resources/sleep-async/target/debug/libsleep_async.so").unwrap();

    let mut factory = DummyFactory {};
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
    let handler = loader.load(&mut factory, addr).unwrap();

    handler.await.unwrap().unwrap();
}
