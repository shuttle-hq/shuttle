use std::process::exit;

use clap::Parser;
use opentelemetry::global;
use shuttle_deployer::{start, start_proxy, Args, DeployLayer, Persistence, RuntimeManager};
use tokio::select;
use tracing::{error, trace};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

const BINARY_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/release/shuttle-runtime"));

// The `multi_thread` is needed to prevent a deadlock in shuttle_service::loader::build_crate() which spawns two threads
// Without this, both threads just don't start up
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    global::set_text_map_propagator(opentelemetry_datadog::DatadogPropagator::new());

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

    let runtime_manager = RuntimeManager::new(
        BINARY_BYTES,
        args.artifacts_path.clone(),
        args.provisioner_address.uri().to_string(),
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
