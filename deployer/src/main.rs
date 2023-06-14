use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::{
    args::Args,
    deployment::persistence::{dal::Sqlite, Persistence},
    runtime_manager::RuntimeManager,
    DeployerService,
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
    match svc.start().await {
        Ok(_) => (),
        Err(err) => error!(
            "error triggered when starting the deployer server {}",
            err.to_string()
        ),
    }
}
