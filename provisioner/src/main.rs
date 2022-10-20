use std::{net::SocketAddr, time::Duration};

use clap::Parser;
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

    println!("starting provisioner on {}", addr);
    Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(30))) // Prevent deployer clients from loosing connection #ENG-219
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
