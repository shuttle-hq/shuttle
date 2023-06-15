use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::{
    args::Args,
    deployment::persistence::{dal::Sqlite, Persistence},
    runtime_manager::RuntimeManager,
    DeployerService, DeployerServiceConfig, DeployerServiceConfigBuilder,
};
use tracing::{error, trace};

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let (persistence, _) = Persistence::from_dal(Sqlite::new(&args.state).await).await;
    setup_tracing(
        tracing_subscriber::registry().with(DeployLayer::new(persistence.clone())),
        "deployer",
    );
    trace!(args = ?args, "parsed args");
    let config: DeployerServiceConfig = DeployerServiceConfigBuilder::default()
        .artifacts_path(args.artifacts_path)
        .auth_uri(args.auth_uri)
        .provisioner_uri(args.provisioner_uri)
        .bind_address(args.address)
        .docker_host(args.docker_host)
        .network_name(args.network_name)
        .prefix(args.prefix)
        .build()
        .expect("to build the deployer service configuration");

    let runtime_manager = RuntimeManager::new(persistence.get_log_sender());
    let svc = DeployerService::new(runtime_manager, persistence, config).await;

    match svc.start().await {
        Ok(_) => (),
        Err(err) => error!(
            "error triggered when starting the deployer server {}",
            err.to_string()
        ),
    }
}
