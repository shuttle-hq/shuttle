use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::{
    start, start_proxy, AbstractProvisionerFactory, Args, DeployLayer, Persistence,
    RuntimeLoggerFactory,
};
use tokio::select;
use tonic::transport::Endpoint;
use tracing::trace;
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

    let provisioner_uri = Endpoint::try_from(format!(
        "http://{}:{}",
        args.provisioner_address, args.provisioner_port
    ))
    .expect("provisioner uri is not valid");

    let abstract_factory =
        AbstractProvisionerFactory::new(provisioner_uri, persistence.clone(), persistence.clone());

    let runtime_logger_factory = RuntimeLoggerFactory::new(persistence.get_log_sender());

    select! {
        _ = start_proxy(args.proxy_address, args.proxy_fqdn.clone(), persistence.clone()) => {},
        _ = start(abstract_factory, runtime_logger_factory, persistence, args) => {},
    }
}
