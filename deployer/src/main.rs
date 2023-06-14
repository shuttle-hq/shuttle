use std::time::Duration;

use clap::Parser;
use shuttle_common::backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_deployer::{
    args::Args,
    deployment::{
        deploy_layer::DeployLayer,
        persistence::{dal::Sqlite, Persistence},
    },
    runtime_manager::RuntimeManager,
    DeployerService,
};
use tracing::trace;
use tracing_subscriber::layer::SubscriberExt;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let (persistence, _) = Persistence::from_dal(Sqlite::new(&args.state).await).await;
    setup_tracing(
        tracing_subscriber::registry().with(DeployLayer::new(persistence.clone())),
        "deployer",
    );
    trace!(args = ?args, "parsed args");

    let runtime_manager = RuntimeManager::new(persistence.get_log_sender());
    let svc = DeployerService::new(
        runtime_manager,
        persistence,
        args.artifacts_path,
        args.address,
        args.docker_host.as_str(),
        args.provisioner_uri,
        args.auth_uri,
        args.network_name,
        args.prefix,
    )
    .await;
}
