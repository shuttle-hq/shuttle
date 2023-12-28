use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;
use shuttle_common::{
    claims::{Claim, ClaimService, InjectPropagation},
    constants::STORAGE_DIRNAME,
    database,
    secrets::Secret,
    DatabaseReadyInfo,
};
use shuttle_proto::provisioner::{
    provisioner_client::ProvisionerClient, DatabaseRequest, QdrantRequest,
};
use shuttle_service::{DeploymentMetadata, Environment, Factory};
use tonic::{transport::Channel, Request};

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    service_name: String,
    provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
    secrets: BTreeMap<String, Secret<String>>,
    env: Environment,
    claim: Option<Claim>,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
        service_name: String,
        secrets: BTreeMap<String, Secret<String>>,
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
        let mut request = Request::new(DatabaseRequest {
            project_name: self.service_name.to_string(),
            db_type: Some(db_type.into()),
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

        Ok(info)
    }

    async fn get_qdrant_connection(
        &mut self,
        url: String,
        api_key: String,
    ) -> Result<QdrantReadyInfo, shuttle_service::Error> {
        let mut request = Request::new(QdrantRequest {
            project_name: self.service_name.to_string(),
            url,
            api_key,
        });

        if let Some(claim) = &self.claim {
            request.extensions_mut().insert(claim.clone());
        }

        let response = self
            .provisioner_client
            .provision_qdrant(request)
            .await
            .map_err(shuttle_service::error::CustomError::new)?
            .into_inner();

        let info: QdrantReadyInfo = response.into();

        // return the connection info
        Ok(info)
    }

    async fn get_secrets(
        &mut self,
    ) -> Result<BTreeMap<String, Secret<String>>, shuttle_service::Error> {
        Ok(self.secrets.clone())
    }

    fn get_metadata(&self) -> DeploymentMetadata {
        DeploymentMetadata {
            env: self.env,
            project_name: self.service_name.to_string(),
            service_name: self.service_name.to_string(),
            storage_path: PathBuf::from(STORAGE_DIRNAME),
        }
    }
}
