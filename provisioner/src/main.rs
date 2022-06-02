use provisioner::provisioner_server::{Provisioner, ProvisionerServer};
use provisioner::{DatabaseRequest, DatabaseResponse};
use tonic::{transport::Server, Status};
use tonic::{Request, Response};

mod provisioner {
    tonic::include_proto!("provisioner");
}

struct MyProvisioner {}

#[tonic::async_trait]
impl Provisioner for MyProvisioner {
    async fn provision_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        println!("request: {:?}", request.into_inner());

        let reply = DatabaseResponse {
            username: "postgres".to_string(),
            password: "tmp".to_string(),
            database_name: "postgres".to_string(),
        };

        Ok(Response::new(reply))
    }
}

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
