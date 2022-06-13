mod persistence;
use persistence::Persistence;

mod tower_service;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let _ = Persistence::new();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8001));

    let deployer = tower_service::Deployer::new();
    let shared = tower::make::Shared::new(deployer);

    log::info!("Binding to and listening at address: {}", addr);

    hyper::Server::bind(&addr)
        .serve(shared)
        .await
        .unwrap_or_else(|_| log::error!("Failed to bind to address: {}", addr));
}
