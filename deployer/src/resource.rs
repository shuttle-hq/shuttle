use shuttle_common::claims::{ClaimLayer, ClaimService, InjectPropagation, InjectPropagationLayer};
use shuttle_proto::provisioner::DatabaseRequest;
use tonic::{
    transport::{Channel, Endpoint},
    Request,
};
use tower::ServiceBuilder;

type ProvisionerClient = shuttle_proto::provisioner::provisioner_client::ProvisionerClient<
    ClaimService<InjectPropagation<Channel>>,
>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),
    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),

    #[error("Not implemented")]
    NotImplemented,
}

type Result<T> = std::result::Result<T, Error>;

// Arguably, this should have its own ResourcePersistence, but this code is intended to be temporary.
#[derive(Clone)]
pub struct ResourceManager {
    provisioner_client: ProvisionerClient,
}

impl ResourceManager {
    pub async fn new(provisioner_address: &Endpoint) -> Result<Self> {
        let channel = provisioner_address.connect().await?;
        let service = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);
        let provisioner_client = ProvisionerClient::new(service);

        Ok(Self { provisioner_client })
    }

    pub async fn delete_resource(
        &mut self,
        project_name: &str,
        resource_type: &shuttle_common::resource::Type,
    ) -> Result<()> {
        match resource_type {
            shuttle_common::resource::Type::Database(database_type) => {
                self.delete_database(project_name, database_type).await
            }

            _ => Err(Error::NotImplemented),
        }
    }

    async fn delete_database(
        &mut self,
        project_name: &str,
        database_type: &shuttle_common::database::Type,
    ) -> Result<()> {
        self.provisioner_client
            .delete_database(Request::new(DatabaseRequest {
                project_name: project_name.to_string(),
                db_type: Some((*database_type).into()),
            }))
            .await?;

        Ok(())
    }
}
