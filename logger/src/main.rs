use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use clap::Parser;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        tracing::{setup_tracing, ExtractPropagationLayer},
    },
    log::Backend,
};
use shuttle_logger::{
    args::Args,
    rate_limiting::{tonic_error, TonicPeerIpKeyExtractor},
    Postgres, Service,
};
use shuttle_proto::logger::logger_server::LoggerServer;
use tonic::transport::Server;
use tower::BoxError;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), Backend::Logger, None);

    trace!(args = ?args, "parsed args");

    // The server can receive no more than 6 requests per peer address per second.
    let governor_config = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(6)
        .use_headers()
        .key_extractor(TonicPeerIpKeyExtractor)
        .finish()
        .unwrap();

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer)
        // This middleware goes above `GovernorLayer` because it will receive errors returned by
        // `GovernorLayer`.
        .layer(HandleErrorLayer::new(|e: BoxError| async move {
            tonic_error(e)
        }))
        .layer(GovernorLayer {
            config: &governor_config,
        });

    let postgres = Postgres::new(&args.db_connection_uri).await;

    let router = server_builder.add_service(LoggerServer::new(Service::new(
        postgres.get_sender(),
        postgres,
    )));

    router.serve(args.address).await.unwrap();
}
