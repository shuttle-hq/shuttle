use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use clap::Parser;
use shuttle_proto::runtime::runtime_server::RuntimeServer;
use shuttle_runtime::{AxumWasm, NextArgs};
use tonic::transport::Server;
use tracing::trace;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = NextArgs::parse();

    // TODO: replace with tracing helper from main branch
    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    trace!(args = ?args, "parsed args");

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let mut server_builder =
        Server::builder().http2_keepalive_interval(Some(Duration::from_secs(60)));

    let axum = AxumWasm::default();
    let svc = RuntimeServer::new(axum);
    let router = server_builder.add_service(svc);

    router.serve(addr).await.unwrap();
}
