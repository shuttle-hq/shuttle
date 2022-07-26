mod deployment;
mod error;
mod handlers;
mod persistence;

use deployment::{Built, DeploymentManager};
use persistence::Persistence;
use tracing::{info, trace};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

use std::net::SocketAddr;

use crate::deployment::deploy_layer::DeployLayer;

const SECRET_KEY: &str = "GATEWAY_SECRET";

#[tokio::main]
async fn main() {
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

    let deployment_manager = DeploymentManager::new(persistence.clone());

    for existing_deployment in persistence.get_all_runnable_deployments().await.unwrap() {
        let built = Built {
            id: existing_deployment.id,
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
