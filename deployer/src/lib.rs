use std::net::SocketAddr;

pub use args::Args;
pub use deployment::{
    deploy_layer::DeployLayer, provisioner_factory::AbstractProvisionerFactory,
    runtime_logger::RuntimeLoggerFactory,
};
use deployment::{provisioner_factory, runtime_logger, Built, DeploymentManager};
pub use persistence::Persistence;
use tracing::info;

mod args;
mod deployment;
mod error;
mod handlers;
mod persistence;

pub async fn start(
    abstract_factory: impl provisioner_factory::AbstractFactory,
    runtime_logger_factory: impl runtime_logger::Factory,
    persistence: Persistence,
    args: Args,
) {
    let deployment_manager = DeploymentManager::new(
        abstract_factory,
        runtime_logger_factory,
        persistence.clone(),
        persistence.clone(),
    );

    for existing_deployment in persistence.get_all_runnable_deployments().await.unwrap() {
        let built = Built {
            id: existing_deployment.id,
            name: existing_deployment.name,
        };
        deployment_manager.run_push(built).await;
    }

    let router = handlers::make_router(persistence, deployment_manager, args.proxy_fqdn);
    let make_service = router.into_make_service();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8001));

    info!("Binding to and listening at address: {}", addr);

    axum::Server::bind(&addr)
        .serve(make_service)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", addr));
}
