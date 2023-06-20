use std::time::Duration;

use clap::Parser;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsServiceServer;
use shuttle_common::backends::{
    auth::JwtAuthenticationLayer,
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_logger::{args::Args, Service, ShuttleLogsOtlp, Sqlite};
use shuttle_proto::{auth::AuthPublicKey, logger::logger_server::LoggerServer};
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), "logger");

    trace!(args = ?args, "parsed args");

    let auth_client = shuttle_proto::auth::client(&args.auth_uri)
        .await
        .expect("auth service should be reachable");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(auth_client)))
        .layer(ExtractPropagationLayer);

    let sqlite = Sqlite::new(&args.state.display().to_string()).await;
    let svc = ShuttleLogsOtlp::new(sqlite.get_sender());
    let svc = LogsServiceServer::new(svc);
    let router = server_builder
        .add_service(svc)
        .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)));

    router.serve(args.address).await.unwrap();
}
