use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use shuttle_common::backends::trace::ExtractPropagationLayer;
use shuttle_proto::runtime::runtime_server::RuntimeServer;
use shuttle_runtime::__internals::{print_version, AxumWasm, NextArgs};
use tonic::transport::Server;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // `--version` overrides any other arguments.
    if std::env::args().any(|arg| arg == "--version") {
        print_version();
        return;
    }

    let args = NextArgs::parse().unwrap();

    println!("parsed args");

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(ExtractPropagationLayer);

    let axum = AxumWasm::default();
    let svc = RuntimeServer::new(axum);
    let router = server_builder.add_service(svc);

    router.serve(addr).await.unwrap();
}
