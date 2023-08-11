use std::time::Duration;

use clap::Parser;
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceServiceServer;
use shuttle_common::backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_logger::{args::Args, Service, ShuttleLogsOtlp, Sqlite};
use shuttle_proto::logger::logger_server::LoggerServer;
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), "logger");

    trace!(args = ?args, "parsed args");

    let db_path = args.state.join("logger.sqlite");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    let sqlite = Sqlite::new(&db_path.display().to_string()).await;
    let svc = ShuttleLogsOtlp::new(sqlite.get_sender());
    let svc = TraceServiceServer::new(svc);
    let router = server_builder
        .add_service(svc)
        .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)));

    router.serve(args.address).await.unwrap();
}
