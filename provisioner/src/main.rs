use std::net::SocketAddr;

use clap::Parser;
use provisioner::{Args, MyProvisioner, ProvisionerServer};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let Args {
        ip,
        port,
        shared_pg_uri,
    } = Args::parse();
    let addr = SocketAddr::new(ip, port);

    let provisioner = MyProvisioner::new(&shared_pg_uri).unwrap();

    println!("starting provisioner on {}", addr);
    Server::builder()
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
