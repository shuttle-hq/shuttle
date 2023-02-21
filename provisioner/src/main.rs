use std::{net::SocketAddr, time::Duration};

use clap::Parser;
use shuttle_common::backends::auth::{public_key_from_auth, JwtAuthenticationLayer};
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

    let public_key_fn = public_key_from_auth(auth_uri).await;

    println!("starting provisioner on {}", addr);
    Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(30))) // Prevent deployer clients from loosing connection #ENG-219
        .layer(JwtAuthenticationLayer::new(public_key_fn))
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
