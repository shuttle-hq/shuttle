use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use shuttle_deployer::{start, start_proxy, Args, DeployLayer, Persistence};
use shuttle_proto::runtime::runtime_client::RuntimeClient;
use tokio::select;
use tracing::{error, trace};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

// The `multi_thread` is needed to prevent a deadlock in shuttle_service::loader::build_crate() which spawns two threads
// Without this, both threads just don't start up
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let (persistence, _) = Persistence::new(&args.state).await;
    let tracer = opentelemetry_datadog::new_pipeline()
        .with_service_name("deployer")
        .with_agent_endpoint("http://datadog-agent:8126")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(DeployLayer::new(persistence.clone()))
        .with(filter_layer)
        .with(fmt_layer)
        .with(opentelemetry)
        .init();

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let runtime_dir = workspace_root.join("target/debug");

    let mut runtime = tokio::process::Command::new(runtime_dir.join("shuttle-runtime"))
        .args(&["--legacy", "--provisioner-address", "http://localhost:8000"])
        .current_dir(&runtime_dir)
        .spawn()
        .unwrap();

    let runtime_client = RuntimeClient::connect("http://127.0.0.1:6001")
        .await
        .unwrap();

    select! {
        _ = start_proxy(args.proxy_address, args.proxy_fqdn.clone(), persistence.clone()) => {
            error!("Proxy stopped.")
        },
        _ = start(persistence, runtime_client, args) => {
            error!("Deployment service stopped.")
        },
        _ = runtime.wait() => {
            error!("Legacy runtime stopped.")
        },
    }

    exit(1);
}
