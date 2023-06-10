use async_trait::async_trait;
use dal::Dal;
use error::Result;
use shuttle_common::{backends::auth::VerifyClaim, claims::Scope};
use shuttle_proto::deployer::{deployer_server::Deployer, DeployRequest, DeployResponse};
use tonic::{Response, Result as TonicResult};
use ulid::Ulid;

pub mod account;
pub mod args;
pub mod dal;
pub mod error;

pub struct DeployerService<D: Dal + Send + Sync + 'static> {
    dal: D,
}

impl<D: Dal + Send + Sync + 'static> DeployerService<D> {
    pub async fn new(dal: D) -> Self {
        Self { dal }
    }

    pub fn dal(&self) -> &D {
        &self.dal
    }

    pub async fn push_deployment(&self, req: DeployRequest) -> Result<()> {
        //TODO: store deployment in persistence
        Ok(())
    }
}

#[async_trait]
impl<D: Dal + Send + Sync + 'static> Deployer for DeployerService<D> {
    async fn deploy(
        &self,
        request: tonic::Request<DeployRequest>,
    ) -> TonicResult<tonic::Response<DeployResponse>, tonic::Status> {
        request.verify(Scope::DeploymentPush)?;
        let request = request.into_inner();
        let result = self.push_deployment(request).await.unwrap();
        Ok(Response::new(DeployResponse {
            deployment_id: Ulid::new().to_string(),
        }))
    }
}
