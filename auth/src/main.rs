use clap::Parser;
use opentelemetry::global;
use tracing::{info, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use shuttle_auth::{start, Args, Commands};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    global::set_text_map_propagator(opentelemetry_datadog::DatadogPropagator::new());

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let tracer = opentelemetry_datadog::new_pipeline()
        .with_service_name("gateway")
        .with_agent_endpoint("http://datadog-agent:8126")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(opentelemetry)
        .init();

    let db_path = args.state.join("authentication.sqlite");

    info!(
        "state db: {}",
        std::fs::canonicalize(&args.state)
            .unwrap()
            .to_string_lossy()
    );

    let db_uri = db_path.to_str().unwrap();

    // If the start command is called, start the auth server with given DB path and address.
    // If the init command is called, do the same but insert an admin user as well.
    match args.command {
        Commands::Start => start(db_uri, args.address, None).await,
        Commands::Init(init_args) => start(db_uri, args.address, Some(init_args)).await,
    }
}
