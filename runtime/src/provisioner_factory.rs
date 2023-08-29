use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;
use shuttle_common::{
    claims::{Claim, ClaimService, InjectPropagation},
    database,
    models::deployment::STORAGE_DIRNAME,
    DatabaseReadyInfo,
};
use shuttle_proto::provisioner::{provisioner_client::ProvisionerClient, DatabaseRequest};
use shuttle_service::{DeploymentMetadata, Environment, Factory, ProjectName};
use tonic::{transport::Channel, Request};
use tracing::info;

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    service_name: ProjectName,
    provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
    secrets: BTreeMap<String, String>,
    env: Environment,
    claim: Option<Claim>,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
        service_name: ProjectName,
        secrets: BTreeMap<String, String>,
        env: Environment,
        claim: Option<Claim>,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            secrets,
            env,
            claim,
        }
    }
}

#[async_trait]
impl Factory for ProvisionerFactory {
    async fn get_db_connection(
        &mut self,
        db_type: database::Type,
    ) -> Result<DatabaseReadyInfo, shuttle_service::Error> {
        info!("Provisioning a {db_type}. This can take a while...");

        let mut request = Request::new(DatabaseRequest {
            project_name: self.service_name.to_string(),
            db_type: Some(db_type.clone().into()),
        });

        if let Some(claim) = &self.claim {
            request.extensions_mut().insert(claim.clone());
        }

        let response = self
            .provisioner_client
            .provision_database(request)
            .await
            .map_err(shuttle_service::error::CustomError::new)?
            .into_inner();

        let info: DatabaseReadyInfo = response.into();

        info!("Done provisioning database");

        Ok(info)
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        Ok(self.secrets.clone())
    }

    fn get_metadata(&self) -> DeploymentMetadata {
        DeploymentMetadata {
            env: self.env,
            project_name: self.service_name.clone(),
            service_name: self.service_name.to_string(),
            storage_path: PathBuf::from(STORAGE_DIRNAME),
        }
    }
}
