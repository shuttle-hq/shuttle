use std::time::Duration;

use clap::Parser;
use shuttle_builder::{args::Args, Service};
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        trace::{setup_tracing, ExtractPropagationLayer},
    },
    log::Backend,
};
use shuttle_proto::builder::builder_server::BuilderServer;
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(
        tracing_subscriber::registry(),
        Backend::Builder,
        Some("nbuild_core=warn,info"),
    );

    trace!(args = ?args, "parsed args");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    let svc = Service::new();
    let svc = BuilderServer::new(svc);
    let router = server_builder.add_service(svc);

    router.serve(args.address).await.unwrap();
}
