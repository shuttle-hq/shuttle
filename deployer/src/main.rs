use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::{
    args::Args, dal::Sqlite, DeployerService, DeployerServiceConfig, DeployerServiceConfigBuilder,
};
use tracing::{error, trace};

#[tokio::main]
async fn main() {
    let args = Args::parse();
    setup_tracing(tracing_subscriber::registry(), "deployer");

    trace!(args = ?args, "parsed args");

    let config: DeployerServiceConfig = DeployerServiceConfigBuilder::default()
        .auth_uri(args.auth_uri)
        .provisioner_uri(args.provisioner_uri)
        .bind_address(args.address)
        .docker_host(args.docker_host)
        .network_name(args.users_network_name)
        .prefix(args.prefix)
        .build()
        .expect("to build the deployer service configuration");

    let svc = DeployerService::new(Sqlite::new(&args.state).await, config).await;

    match svc.start().await {
        Ok(_) => (),
        Err(err) => error!(
            "error triggered when starting the deployer server {}",
            err.to_string()
        ),
    }
}
