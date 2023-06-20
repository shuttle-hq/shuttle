use std::time::Duration;

use clap::Parser;
use shuttle_builder::{args::Args, Service};
use shuttle_common::backends::{
    auth::JwtAuthenticationLayer,
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_proto::{auth::AuthPublicKey, builder::builder_server::BuilderServer};
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), "resource-recorder");

    trace!(args = ?args, "parsed args");

    let auth_client = shuttle_proto::auth::client(&args.auth_uri)
        .await
        .expect("auth service should be reachable");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(auth_client)))
        .layer(ExtractPropagationLayer);

    let svc = Service::new();
    let svc = BuilderServer::new(svc);
    let router = server_builder.add_service(svc);

    router.serve(args.address).await.unwrap();
}
