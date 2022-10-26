use std::net::{Ipv4Addr, SocketAddr};

use clap::Parser;
use shuttle_next::{args::Args, Next};
use shuttle_runtime_proto::runtime::runtime_server::RuntimeServer;
use tonic::transport::Server;
use tracing::trace;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
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

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8000);
    let next = Next::new();
    Server::builder()
        .add_service(RuntimeServer::new(next))
        .serve(addr)
        .await
        .unwrap();
}
