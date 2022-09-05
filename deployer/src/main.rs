use clap::Parser;
use shuttle_deployer::{
    start, AbstractProvisionerFactory, Args, DeployLayer, Persistence, RuntimeLoggerFactory,
};
use shuttle_proto::provisioner::provisioner_client::ProvisionerClient;
use tonic::transport::Endpoint;
use tracing::trace;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

// The `multi_thread` is needed to prevent a deadlock in shutlte_service::loader::build_crate() which spawns two threads
// Without this, both threads just don't start up
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let (persistence, _) = Persistence::new().await;
    let tracer = opentelemetry_datadog::new_pipeline()
        .with_service_name("deployer")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(DeployLayer::new(persistence.clone()))
        .with(filter_layer)
        .with(fmt_layer)
        .with(opentelemetry)
        .init();

    let provisioner_uri = Endpoint::try_from(format!(
        "http://{}:{}",
        args.provisioner_address, args.provisioner_port
    ))
    .expect("provisioner uri is not valid");

    let provisioner_client = ProvisionerClient::connect(provisioner_uri)
        .await
        .expect("failed to connect to provisioner");

    let abstract_factory = AbstractProvisionerFactory::new(
        provisioner_client,
        persistence.clone(),
        persistence.clone(),
    );

    let runtime_logger_factory = RuntimeLoggerFactory::new(persistence.get_log_sender());

    start(abstract_factory, runtime_logger_factory, persistence, args).await;
}
