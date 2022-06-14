mod deployment;
mod persistence;
mod tower_service;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8001));

    let deployer = tower::ServiceBuilder::new()
        .layer(tower_service::middleware::LoggingLayer(log::Level::Debug))
        .service(tower_service::Deployer::new().await);

    let shared = tower::make::Shared::new(deployer);

    log::info!("Binding to and listening at address: {}", addr);

    hyper::Server::bind(&addr)
        .serve(shared)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", addr));
}
