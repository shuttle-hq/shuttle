use std::time::Duration;

use clap::Parser;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        trace::{setup_tracing, ExtractPropagationLayer},
    },
    log::Backend,
};
use shuttle_logger::{args::Args, Postgres, Service};
use shuttle_proto::logger::logger_server::LoggerServer;
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), Backend::Logger, None);

    trace!(args = ?args, "parsed args");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    let postgres = Postgres::new(&args.db_connection_uri).await;

    let router = server_builder.add_service(LoggerServer::new(Service::new(
        postgres.get_sender(),
        postgres,
    )));

    router.serve(args.address).await.unwrap();
}
