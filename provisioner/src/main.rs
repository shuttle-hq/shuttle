use std::{net::SocketAddr, sync::Arc, time::Duration};

use clap::Parser;
use shuttle_common::backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    cache::CacheManager,
};
use shuttle_provisioner::{Args, MyProvisioner, ProvisionerServer};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let Args {
        ip,
        port,
        shared_pg_uri,
        shared_mongodb_uri,
        fqdn,
        internal_pg_address,
        internal_mongodb_address,
        auth_uri,
    } = Args::parse();
    let addr = SocketAddr::new(ip, port);

    let provisioner = MyProvisioner::new(
        &shared_pg_uri,
        &shared_mongodb_uri,
        fqdn.to_string(),
        internal_pg_address,
        internal_mongodb_address,
    )
    .await
    .unwrap();

    let public_key_cache_manager = CacheManager::new();

    println!("starting provisioner on {}", addr);
    Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(30))) // Prevent deployer clients from loosing connection #ENG-219
        .layer(JwtAuthenticationLayer::new(
            AuthPublicKey::new(auth_uri),
            Arc::new(Box::new(public_key_cache_manager)),
        ))
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
