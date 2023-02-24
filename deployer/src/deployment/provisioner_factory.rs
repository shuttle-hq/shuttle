use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;
use shuttle_common::{
    backends::auth::{Claim, ClaimLayer, ClaimService},
    database, DatabaseReadyInfo,
};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::{Environment, Factory, ServiceName};
use thiserror::Error;
use tonic::{
    transport::{Channel, Endpoint},
    Request,
};
use tower::ServiceBuilder;
use tracing::{debug, info, trace};
use uuid::Uuid;

use crate::persistence::{Resource, ResourceManager, ResourceType, SecretGetter};

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
        claim: Option<Claim>,
    ) -> Result<Self::Output, Self::Error>;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractProvisionerFactory<R: ResourceManager, S: SecretGetter> {
    provisioner_uri: Endpoint,
    resource_manager: R,
    secret_getter: S,
}

#[async_trait]
impl<R: ResourceManager, S: SecretGetter> AbstractFactory for AbstractProvisionerFactory<R, S> {
    type Output = ProvisionerFactory<R, S>;
    type Error = ProvisionerError;

    async fn get_factory(
        &self,
        service_name: ServiceName,
        service_id: Uuid,
        deployment_id: Uuid,
        storage_manager: StorageManager,
        claim: Option<Claim>,
    ) -> Result<Self::Output, Self::Error> {
        let channel = self.provisioner_uri.clone().connect().await?;
        let channel = ServiceBuilder::new().layer(ClaimLayer).service(channel);

        let provisioner_client = ProvisionerClient::new(channel);

        Ok(ProvisionerFactory {
            provisioner_client,
            service_name,
            service_id,
            deployment_id,
            storage_manager,
            resource_manager: self.resource_manager.clone(),
            secret_getter: self.secret_getter.clone(),
            claim,
            info: None,
            secrets: None,
        })
    }
}

impl<R: ResourceManager, S: SecretGetter> AbstractProvisionerFactory<R, S> {
    pub fn new(provisioner_uri: Endpoint, resource_manager: R, secret_getter: S) -> Self {
        Self {
            provisioner_uri,
            resource_manager,
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
pub struct ProvisionerFactory<R: ResourceManager, S: SecretGetter> {
    service_name: ServiceName,
    service_id: Uuid,
    deployment_id: Uuid,
    storage_manager: StorageManager,
    provisioner_client: ProvisionerClient<ClaimService<Channel>>,
    info: Option<DatabaseReadyInfo>,
    resource_manager: R,
    secret_getter: S,
    secrets: Option<BTreeMap<String, String>>,
    claim: Option<Claim>,
}

#[async_trait]
impl<R: ResourceManager, S: SecretGetter> Factory for ProvisionerFactory<R, S> {
    async fn get_db_connection_string(
        &mut self,
        db_type: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        if let Some(ref info) = self.info {
            debug!("A database has already been provisioned for this deployment, so reusing it");
            return Ok(info.connection_string_private());
        }

        let r#type = ResourceType::Database(db_type.clone().into());

        // Try to get the database info from provisioner if possible
        let info = if let Some(claim) = self.claim.clone() {
            info!("Provisioning a {db_type} on the shuttle servers. This can take a while...");

            let db_type: DbType = db_type.into();

            let mut request = Request::new(DatabaseRequest {
                project_name: self.service_name.to_string(),
                db_type: Some(db_type),
            });

            request.extensions_mut().insert(claim);

            let response = self
                .provisioner_client
                .provision_database(request)
                .await
                .map_err(shuttle_service::error::CustomError::new)?
                .into_inner();

            let info: DatabaseReadyInfo = response.into();

            self.resource_manager
                .insert_resource(&Resource {
                    service_id: self.service_id,
                    r#type,
                    data: serde_json::to_value(&info).map_err(|err| {
                        shuttle_service::Error::Database(format!(
                            "failed to convert DatabaseReadyInfo to json: {err}",
                        ))
                    })?,
                })
                .await
                .map_err(|err| {
                    shuttle_service::Error::Database(format!("failed to store resource: {err}"))
                })?;

            info
        } else {
            info!("Getting a {db_type} from a previous provision");

            let resources = self
                .resource_manager
                .get_resources(&self.service_id)
                .await
                .map_err(|err| {
                    shuttle_service::Error::Database(format!("failed to get resources: {err}"))
                })?;

            let info = resources.into_iter().find_map(|resource| {
                if resource.r#type == r#type {
                    Some(resource.data)
                } else {
                    None
                }
            });

            if let Some(info) = info {
                serde_json::from_value(info).map_err(|err| {
                    shuttle_service::Error::Database(format!(
                        "failed to convert json to DatabaseReadyInfo: {err}",
                    ))
                })?
            } else {
                return Err(shuttle_service::Error::Database(
                    "could not find resource from past resources".to_string(),
                ));
            }
        };

        let conn_str = info.connection_string_private();
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

    fn get_environment(&self) -> Environment {
        Environment::Production
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
