use provisioner::{MyProvisioner, ProvisionerServer};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:5001".parse()?;
    let provisioner = MyProvisioner {};

    Server::builder()
        .add_service(ProvisionerServer::new(provisioner))
        .serve(addr)
        .await?;

    Ok(())
}
