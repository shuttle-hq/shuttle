use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use shuttle_common::{
    claims::{ClaimService, InjectPropagation},
    database,
    storage_manager::StorageManager,
    DatabaseReadyInfo,
};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::{Environment, Factory, ServiceName};
use tonic::{transport::Channel, Request};
use tracing::{debug, info, trace};

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    service_name: ServiceName,
    storage_manager: Arc<dyn StorageManager>,
    provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
    info: Option<DatabaseReadyInfo>,
    secrets: BTreeMap<String, String>,
    env: Environment,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<ClaimService<InjectPropagation<Channel>>>,
        service_name: ServiceName,
        secrets: BTreeMap<String, String>,
        storage_manager: Arc<dyn StorageManager>,
        env: Environment,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            storage_manager,
            info: None,
            secrets,
            env,
        }
    }
}

#[async_trait]
impl Factory for ProvisionerFactory {
    async fn get_db_connection_string(
        &mut self,
        db_type: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        info!("Provisioning a {db_type}. This can take a while...");

        if let Some(ref info) = self.info {
            debug!("A database has already been provisioned for this deployment, so reusing it");
            return Ok(info.connection_string_private());
        }

        let db_type: DbType = db_type.into();

        let request = Request::new(DatabaseRequest {
            project_name: self.service_name.to_string(),
            db_type: Some(db_type),
        });

        let response = self
            .provisioner_client
            .provision_database(request)
            .await
            .map_err(shuttle_service::error::CustomError::new)?
            .into_inner();

        let info: DatabaseReadyInfo = response.into();
        let conn_str = info.connection_string_private();

        self.info = Some(info);

        info!("Done provisioning database");
        trace!("giving a DB connection string: {}", conn_str);
        Ok(conn_str)
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        Ok(self.secrets.clone())
    }

    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
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

    fn get_environment(&self) -> shuttle_service::Environment {
        self.env
    }
}
