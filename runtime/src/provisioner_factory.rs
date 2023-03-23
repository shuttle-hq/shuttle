use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use shuttle_common::{
    claims::{Claim, ClaimService, InjectPropagation},
    database,
    resource::{self, ResourceInfo},
    storage_manager::StorageManager,
    DatabaseReadyInfo,
};
use shuttle_proto::provisioner::{provisioner_client::ProvisionerClient, DatabaseRequest};
use shuttle_service::{Environment, Factory, ServiceName};
use tokio::sync::Mutex;
use tonic::{transport::Channel, Request};
use tracing::{debug, info, trace};

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    service_name: ServiceName,
    storage_manager: Arc<dyn StorageManager>,
    provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
    secrets: BTreeMap<String, String>,
    env: Environment,
    claim: Option<Claim>,
    resources: Arc<Mutex<Vec<resource::Response>>>,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
        service_name: ServiceName,
        secrets: BTreeMap<String, String>,
        storage_manager: Arc<dyn StorageManager>,
        env: Environment,
        claim: Option<Claim>,
        resources: Arc<Mutex<Vec<resource::Response>>>,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            storage_manager,
            secrets,
            env,
            claim,
            resources,
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

        // if let Some(info) = self
        //     .resources
        //     .lock()
        //     .await
        //     .iter()
        //     .find(|resource| resource.r#type == resource::Type::Database(db_type.clone()))
        // {
        //     debug!("A database has already been provisioned for this deployment, so reusing it");

        //     let resource = info.get_resource_info();
        //     return Ok(resource.connection_string_private());
        // }

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

        self.resources.lock().await.push(resource::Response {
            r#type: resource::Type::Database(db_type),
            data: serde_json::to_value(&info).expect("to convert DB info"),
        });

        info!("Done provisioning database");

        Ok(info)
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        Ok(self.secrets.clone())
    }

    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }

    fn get_environment(&self) -> shuttle_service::Environment {
        self.env
    }

    fn get_build_path(&self) -> Result<PathBuf, shuttle_service::Error> {
        self.storage_manager
            .service_build_path(self.service_name.as_str())
            .map_err(Into::into)
    }

    fn get_storage_path(&self) -> Result<PathBuf, shuttle_service::Error> {
        self.storage_manager
            .service_storage_path(self.service_name.as_str())
            .map_err(Into::into)
    }
}
