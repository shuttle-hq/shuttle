use std::collections::BTreeMap;

use async_trait::async_trait;
use shuttle_common::{database, DatabaseReadyInfo};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::{Factory, ServiceName};
use tonic::{transport::Channel, Request};
use tracing::debug;
use uuid::Uuid;

use crate::persistence::{Resource, ResourceRecorder, ResourceType, SecretGetter};

/// Trait to make it easy to get a factory (service locator) for each service being started
pub trait AbstractFactory: Send + 'static {
    type Output: Factory;

    /// Get a factory for a specific service
    fn get_factory(&self, service_name: ServiceName, service_id: Uuid) -> Self::Output;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractProvisionerFactory<R: ResourceRecorder, S: SecretGetter> {
    provisioner_client: ProvisionerClient<Channel>,
    resource_recorder: R,
    secret_getter: S,
}

impl<R: ResourceRecorder, S: SecretGetter> AbstractFactory for AbstractProvisionerFactory<R, S> {
    type Output = ProvisionerFactory<R, S>;

    fn get_factory(&self, service_name: ServiceName, service_id: Uuid) -> Self::Output {
        ProvisionerFactory::new(
            self.provisioner_client.clone(),
            service_name,
            service_id,
            self.resource_recorder.clone(),
            self.secret_getter.clone(),
        )
    }
}

impl<R: ResourceRecorder, S: SecretGetter> AbstractProvisionerFactory<R, S> {
    pub fn new(
        provisioner_client: ProvisionerClient<Channel>,
        resource_recorder: R,
        secret_getter: S,
    ) -> Self {
        Self {
            provisioner_client,
            resource_recorder,
            secret_getter,
        }
    }
}

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory<R: ResourceRecorder, S: SecretGetter> {
    service_name: ServiceName,
    service_id: Uuid,
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
        resource_recorder: R,
        secret_getter: S,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            service_id,
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
        if let Some(ref info) = self.info {
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

        debug!("giving a DB connection string: {}", conn_str);
        Ok(conn_str)
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        if let Some(ref secrets) = self.secrets {
            Ok(secrets.clone())
        } else {
            let iter = self
                .secret_getter
                .get_secrets(&self.service_id)
                .await
                .map_err(shuttle_service::error::CustomError::new)?
                .into_iter()
                .map(|secret| (secret.key, secret.value));

            let secrets = BTreeMap::from_iter(iter);
            self.secrets = Some(secrets.clone());

            Ok(secrets)
        }
    }

    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }
}
