use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;
use shuttle_common::{database, DatabaseReadyInfo};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::{Factory, ServiceName};
use thiserror::Error;
use tonic::{
    transport::{Channel, Endpoint},
    Request,
};
use tracing::{debug, info, trace};
use uuid::Uuid;

use crate::persistence::{Resource, ResourceRecorder, ResourceType, SecretGetter};

use super::storage_manager::StorageManager;

/// Trait to make it easy to get a factory (service locator) for each service being started
#[async_trait]
pub trait AbstractFactory: Send + Sync + 'static {
    type Output: Factory;
    type Error: std::error::Error;

    /// Get a factory for a specific service
    async fn get_factory(
        &self,
        service_name: ServiceName,
        service_id: Uuid,
        deployment_id: Uuid,
        storage_manager: StorageManager,
    ) -> Result<Self::Output, Self::Error>;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractProvisionerFactory<R: ResourceRecorder, S: SecretGetter> {
    provisioner_uri: Endpoint,
    resource_recorder: R,
    secret_getter: S,
}

#[async_trait]
impl<R: ResourceRecorder, S: SecretGetter> AbstractFactory for AbstractProvisionerFactory<R, S> {
    type Output = ProvisionerFactory<R, S>;
    type Error = ProvisionerError;

    async fn get_factory(
        &self,
        service_name: ServiceName,
        service_id: Uuid,
        deployment_id: Uuid,
        storage_manager: StorageManager,
    ) -> Result<Self::Output, Self::Error> {
        let provisioner_client = ProvisionerClient::connect(self.provisioner_uri.clone()).await?;

        Ok(ProvisionerFactory::new(
            provisioner_client,
            service_name,
            service_id,
            deployment_id,
            storage_manager,
            self.resource_recorder.clone(),
            self.secret_getter.clone(),
        ))
    }
}

impl<R: ResourceRecorder, S: SecretGetter> AbstractProvisionerFactory<R, S> {
    pub fn new(provisioner_uri: Endpoint, resource_recorder: R, secret_getter: S) -> Self {
        Self {
            provisioner_uri,
            resource_recorder,
            secret_getter,
        }
    }
}

#[derive(Error, Debug)]
pub enum ProvisionerError {
    #[error("failed to connect to provisioner: {0}")]
    TonicClient(#[from] tonic::transport::Error),
}

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory<R: ResourceRecorder, S: SecretGetter> {
    service_name: ServiceName,
    service_id: Uuid,
    deployment_id: Uuid,
    storage_manager: StorageManager,
    provisioner_client: ProvisionerClient<Channel>,
    info: Option<DatabaseReadyInfo>,
    resource_recorder: R,
    secret_getter: S,
    secrets: Option<BTreeMap<String, String>>,
}

impl<R: ResourceRecorder, S: SecretGetter> ProvisionerFactory<R, S> {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<Channel>,
        service_name: ServiceName,
        service_id: Uuid,
        deployment_id: Uuid,
        storage_manager: StorageManager,
        resource_recorder: R,
        secret_getter: S,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            service_id,
            deployment_id,
            storage_manager,
            info: None,
            resource_recorder,
            secret_getter,
            secrets: None,
        }
    }
}

#[async_trait]
impl<R: ResourceRecorder, S: SecretGetter> Factory for ProvisionerFactory<R, S> {
    async fn get_db_connection_string(
        &mut self,
        db_type: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        info!("Provisioning a {db_type} on the shuttle servers. This can take a while...");

        if let Some(ref info) = self.info {
            debug!("A database has already been provisioned for this deployment, so reusing it");
            return Ok(info.connection_string_private());
        }

        let r#type = ResourceType::Database(db_type.clone().into());
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

        self.resource_recorder
            .insert_resource(&Resource {
                service_id: self.service_id,
                r#type,
                data: serde_json::to_value(&info).unwrap(),
            })
            .await
            .unwrap();

        self.info = Some(info);

        info!("Done provisioning database");
        trace!("giving a DB connection string: {}", conn_str);
        Ok(conn_str)
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        if let Some(ref secrets) = self.secrets {
            debug!("Returning previously fetched secrets");
            Ok(secrets.clone())
        } else {
            info!("Fetching secrets for deployment");
            let iter = self
                .secret_getter
                .get_secrets(&self.service_id)
                .await
                .map_err(shuttle_service::error::CustomError::new)?
                .into_iter()
                .map(|secret| (secret.key, secret.value));

            let secrets = BTreeMap::from_iter(iter);
            self.secrets = Some(secrets.clone());

            info!("Done fetching secrets");
            Ok(secrets)
        }
    }

    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }

    fn get_build_path(&self) -> PathBuf {
        self.storage_manager
            .service_build_path(self.service_name.as_str())
    }

    fn get_storage_path(&self) -> PathBuf {
        self.storage_manager
            .deployment_storage_path(self.service_name.as_str(), &self.deployment_id)
    }
}
