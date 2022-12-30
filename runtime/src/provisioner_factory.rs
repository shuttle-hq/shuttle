use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;
use shuttle_common::{database, storage_manager::StorageManager, DatabaseReadyInfo};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::{Factory, ServiceName};
use tonic::{transport::Channel, Request};
use tracing::{debug, info, trace};
use uuid::Uuid;

/// Trait to make it easy to get a factory (service locator) for each service being started
pub trait AbstractFactory<S: StorageManager>: Send + 'static {
    type Output: Factory;

    /// Get a factory for a specific service
    fn get_factory(
        &self,
        service_name: ServiceName,
        deployment_id: Uuid,
        secrets: BTreeMap<String, String>,
        storage_manager: S,
    ) -> Self::Output;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractProvisionerFactory {
    provisioner_client: ProvisionerClient<Channel>,
}

impl<S> AbstractFactory<S> for AbstractProvisionerFactory
where
    S: StorageManager,
{
    type Output = ProvisionerFactory<S>;

    fn get_factory(
        &self,
        service_name: ServiceName,
        deployment_id: Uuid,
        secrets: BTreeMap<String, String>,
        storage_manager: S,
    ) -> Self::Output {
        ProvisionerFactory::new(
            self.provisioner_client.clone(),
            service_name,
            deployment_id,
            secrets,
            storage_manager,
        )
    }
}

impl AbstractProvisionerFactory {
    pub fn new(provisioner_client: ProvisionerClient<Channel>) -> Self {
        Self { provisioner_client }
    }
}

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory<S>
where
    S: StorageManager,
{
    service_name: ServiceName,
    deployment_id: Uuid,
    storage_manager: S,
    provisioner_client: ProvisionerClient<Channel>,
    info: Option<DatabaseReadyInfo>,
    secrets: BTreeMap<String, String>,
}

impl<S> ProvisionerFactory<S>
where
    S: StorageManager,
{
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<Channel>,
        service_name: ServiceName,
        deployment_id: Uuid,
        secrets: BTreeMap<String, String>,
        storage_manager: S,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            deployment_id,
            storage_manager,
            info: None,
            secrets,
        }
    }
}

#[async_trait]
impl<S> Factory for ProvisionerFactory<S>
where
    S: StorageManager + Sync + Send,
{
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
            .deployment_storage_path(self.service_name.as_str(), &self.deployment_id)
            .map_err(Into::into)
    }
}
