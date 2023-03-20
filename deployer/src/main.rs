use std::process::exit;

use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::{start, start_proxy, Args, DeployLayer, Persistence, RuntimeManager};
use tokio::select;
use tracing::{error, trace};
use tracing_subscriber::prelude::*;

// The `multi_thread` is needed to prevent a deadlock in shuttle_service::loader::build_crate() which spawns two threads
// Without this, both threads just don't start up
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    let (persistence, _) = Persistence::new(&args.state).await;
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

    select! {
        _ = start_proxy(args.proxy_address, args.proxy_fqdn.clone(), persistence.clone()) => {
            error!("Proxy stopped.")
        },
        _ = start(persistence, runtime_manager, args) => {
            error!("Deployment service stopped.")
        },
    }

    exit(1);
}
