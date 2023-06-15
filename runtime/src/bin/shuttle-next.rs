use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use shuttle_common::backends::tracing::{setup_tracing, ExtractPropagationLayer};
use shuttle_proto::runtime::runtime_server::RuntimeServer;
use shuttle_runtime::{AxumWasm, NextArgs};
use tonic::transport::Server;
use tracing::trace;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    for arg in std::env::args() {
        if arg == "--version" {
            print_version();
            return;
        }
    }

    let args = NextArgs::parse().unwrap();

    setup_tracing(tracing_subscriber::registry(), "shuttle-next");

    trace!(args = ?args, "parsed args");

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(ExtractPropagationLayer);

    let axum = AxumWasm::default();
    let svc = RuntimeServer::new(axum);
    let router = server_builder.add_service(svc);

    router.serve(addr).await.unwrap();
}

fn print_version() {
    // same way `clap` gets these
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");

    println!("{name} {version}");
}
