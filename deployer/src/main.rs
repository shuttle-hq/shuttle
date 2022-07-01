mod args;
mod deployment;
mod error;
mod handlers;
mod persistence;

use clap::Parser;
use deployment::{Built, DeploymentManager};
use persistence::Persistence;
use proto::provisioner::provisioner_client::ProvisionerClient;
use tonic::transport::Endpoint;
use tracing::{info, trace};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::args::Args;
use crate::deployment::deploy_layer::DeployLayer;
use crate::deployment::{AbstractProvisionerFactory, RuntimeLoggerFactory};

const SECRET_KEY: &str = "GATEWAY_SECRET";

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let gateway_secret = std::env::var(SECRET_KEY).unwrap_or_else(|_| {
        panic!(
            "No gateway secret specified with environment variable {}",
            SECRET_KEY
        )
    });
    trace!("{SECRET_KEY} = {gateway_secret}");

    let addr = SocketAddr::from(([127, 0, 0, 1], 8001));

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let (persistence, _) = Persistence::new().await;
    tracing_subscriber::registry()
        .with(DeployLayer::new(persistence.clone()))
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let provisioner_uri = Endpoint::try_from(format!(
        "http://{}:{}",
        args.provisioner_address, args.provisioner_port
    ))
    .expect("provisioner uri is not valid");

    let provisioner_client = ProvisionerClient::connect(provisioner_uri)
        .await
        .expect("failed to connect to provisioner");

    let abstract_factory =
        AbstractProvisionerFactory::new(provisioner_client, args.provisioner_address);

    let runtime_logger_factory = RuntimeLoggerFactory::new(persistence.get_log_sender());

    let deployment_manager = DeploymentManager::new(abstract_factory, runtime_logger_factory);

    for existing_deployment in persistence.get_all_runnable_deployments().await.unwrap() {
        let built = Built {
            name: existing_deployment.name,
            so_path: PathBuf::new(),
        };
        deployment_manager.run_push(built).await;
    }

    let router = handlers::make_router(persistence, deployment_manager);
    let make_service = router.into_make_service();

    info!("Binding to and listening at address: {}", addr);

    axum::Server::bind(&addr)
        .serve(make_service)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", addr));
}
