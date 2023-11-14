use std::time::Duration;

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
    rate_limiting::{tonic_error, TonicPeerIpKeyExtractor, BURST_SIZE, REFRESH_INTERVAL},
    Postgres, Service,
};
use shuttle_proto::logger::logger_server::LoggerServer;
use tonic::transport::Server;
use tower::ServiceBuilder;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), Backend::Logger, None);

    trace!(args = ?args, "parsed args");

    let governor_config = GovernorConfigBuilder::default()
        // Regenerate capacity at a rate of 2 requests per second, meaning the maximum capacity
        // for sustained traffic is 2 RPS per peer address.
        .per_millisecond(REFRESH_INTERVAL)
        // Allow bursts of up to 6 requests, when any burst capacity is used, it will regenerate
        // one element at a time at the rate set above.
        .burst_size(BURST_SIZE)
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
        .layer(
            ServiceBuilder::new()
                // This middleware goes above `GovernorLayer` because it will receive errors returned by
                // `GovernorLayer`.
                .map_err(tonic_error)
                .layer(GovernorLayer {
                    config: &governor_config,
                }),
        );

    let postgres = Postgres::new(&args.db_connection_uri).await;

    let router = server_builder.add_service(LoggerServer::new(Service::new(
        postgres.get_sender(),
        postgres,
    )));

    router.serve(args.address).await.unwrap();
}
