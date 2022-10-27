use std::net::{Ipv4Addr, SocketAddr};

use clap::Parser;
use shuttle_proto::runtime::runtime_server::RuntimeServer;
use shuttle_runtime::{Args, Legacy, Next};
use tonic::transport::Server;
use tracing::trace;
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

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8000);
    let router = if args.legacy {
        let legacy = Legacy::new();
        let svc = RuntimeServer::new(legacy);
        Server::builder().add_service(svc)
    } else {
        let next = Next::new();
        let svc = RuntimeServer::new(next);
        Server::builder().add_service(svc)
    };

    router.serve(addr).await.unwrap();
}
