use provisioner::provisioner_server::Provisioner;
pub use provisioner::provisioner_server::ProvisionerServer;
use provisioner::{DatabaseRequest, DatabaseResponse};
use tonic::{Request, Response, Status};

pub mod provisioner {
    tonic::include_proto!("provisioner");
}

pub struct MyProvisioner {}

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
