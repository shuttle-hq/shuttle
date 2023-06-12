use std::time::Duration;

use clap::Parser;
use shuttle_common::backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_deployer::{
    args::Args,
    dal::Sqlite,
    deployment::persistence::{dal::Sqlite, Persistence},
    runtime_manager::RuntimeManager,
    DeployerService,
};
use shuttle_proto::deployer::deployer_server::DeployerServer;
use tonic::transport::Server;
use tracing::{error, trace};

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

    let (persistence, _) = Persistence::from_dal(Sqlite::new(&args.state)).await;
    setup_tracing(
        tracing_subscriber::registry().with(DeployLayer::new(persistence.clone())),
        "deployer",
    );

    let runtime_manager = RuntimeManager::new(
        args.artifacts_path.clone(),
        args.provisioner_address.uri().to_string(),
        Some(args.auth_uri.to_string()),
        persistence.get_log_sender(),
    );

    let svc = DeployerService::new(
        runtime_manager,
        persistence,
        args.artifacts_path,
        args.address,
        args.gateway_uri,
    )
    .await;
}
