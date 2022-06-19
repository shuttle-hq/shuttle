mod deployment;
mod error;
mod handlers;
mod persistence;

use deployment::{Built, DeploymentManager, DeploymentState};
use persistence::Persistence;

use std::net::SocketAddr;

const SECRET_KEY: &str = "GATEWAY_SECRET";

#[tokio::main]
async fn main() {
    env_logger::init();

    let gateway_secret = std::env::var(SECRET_KEY).unwrap_or_else(|_| {
        panic!(
            "No gateway secret specified with environment variable {}",
            SECRET_KEY
        )
    });
    log::trace!("{SECRET_KEY} = {gateway_secret}");

    let addr = SocketAddr::from(([127, 0, 0, 1], 8001));

    let persistence = Persistence::new().await;
    let deployment_manager = DeploymentManager::new(persistence.clone());

    for existing_deployment in persistence.get_all_runnable_deployments().await.unwrap() {
        let built = Built {
            name: existing_deployment.name,
            state: DeploymentState::Built,
        };
        deployment_manager.run_push(built).await;
    }

    let router = handlers::make_router(persistence, deployment_manager);
    let make_service = router.into_make_service();

    log::info!("Binding to and listening at address: {}", addr);

    axum::Server::bind(&addr)
        .serve(make_service)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", addr));
}
