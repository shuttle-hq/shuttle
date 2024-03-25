use std::{net::SocketAddr, time::Duration};

use clap::Parser;
use shuttle_backends::{
    auth::{AuthPublicKey, JwtAuthenticationLayer},
    trace::setup_tracing,
};
use shuttle_common::{extract_propagation::ExtractPropagationLayer, log::Backend};
use shuttle_provisioner::{Args, ProvisionerServer, ShuttleProvisioner};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_tracing(tracing_subscriber::registry(), Backend::Provisioner);

    let Args {
        ip,
        port,
        shared_pg_uri,
        shared_mongodb_uri,
        fqdn,
        internal_pg_address,
        internal_mongodb_address,
        auth_uri,
        gateway_uri,
        resource_recorder_uri,
    } = Args::parse();
    let addr = SocketAddr::new(ip, port);

    let provisioner = ShuttleProvisioner::new(
        &shared_pg_uri,
        &shared_mongodb_uri,
        fqdn.to_string(),
        internal_pg_address,
        internal_mongodb_address,
        resource_recorder_uri,
        gateway_uri,
    )
    .await
    .unwrap();

    println!("starting provisioner on {}", addr);
    Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(30))) // Prevent deployer clients from loosing connection #ENG-219
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(auth_uri)))
        .layer(ExtractPropagationLayer)
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
