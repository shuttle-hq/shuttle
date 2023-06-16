use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

// use clap::Parser;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsServiceServer;
use shuttle_common::backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_logger::ShuttleLogsOtlp;
use tonic::transport::Server;
// use tracing::trace;

#[tokio::main]
async fn main() {
    // let args = Args::parse();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 4317);

    setup_tracing(tracing_subscriber::registry(), "logger");

    // trace!(args = ?args, "parsed args");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        // .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
        //     args.auth_uri,
        // )))
        .layer(ExtractPropagationLayer);

    let svc = ShuttleLogsOtlp::new();
    let svc = LogsServiceServer::new(svc);
    let router = server_builder.add_service(svc);

    // router.serve(args.address).await.unwrap();
    router.serve(addr).await.unwrap();
}
