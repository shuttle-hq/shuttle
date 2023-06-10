use std::time::Duration;

use clap::Parser;
use shuttle_common::backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_deployer::{args::Args, dal::Sqlite, DeployerService};
use shuttle_proto::deployer::deployer_server::DeployerServer;
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), "deployer");

    // Configure the deployer router.
    let mut router_builder = RouterBuilder::new(&args.auth_uri).await;
    if args.local {
        router_builder = router_builder.with_local_admin_layer();
    }
    trace!(args = ?args, "parsed args");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    let svc = DeployerService::new(Sqlite::new(&args.state.display().to_string()).await).await;
    let svc = DeployerServer::new(svc);
    let router = server_builder.add_service(svc);

    router
        .serve(args.address)
        .await
        .expect("to serve on address");
}
