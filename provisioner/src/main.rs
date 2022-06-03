use provisioner::{MyProvisioner, ProvisionerServer};
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr = "0.0.0.0:5001".parse()?;
    info!("starting provisioner on {}", addr);

    let provisioner = MyProvisioner::new("postgres://postgres:password@localhost").unwrap();

    Server::builder()
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
