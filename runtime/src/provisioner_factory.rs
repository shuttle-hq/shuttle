use std::collections::BTreeMap;

use async_trait::async_trait;
use shuttle_common::{database, DatabaseReadyInfo};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::{Factory, ServiceName};
use tonic::{transport::Channel, Request};
use tracing::{debug, info, trace};

/// Trait to make it easy to get a factory (service locator) for each service being started
pub trait AbstractFactory: Send + 'static {
    type Output: Factory;

    /// Get a factory for a specific service
    fn get_factory(&self, service_name: ServiceName) -> Self::Output;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractProvisionerFactory {
    provisioner_client: ProvisionerClient<Channel>,
}

impl AbstractFactory for AbstractProvisionerFactory {
    type Output = ProvisionerFactory;

    fn get_factory(&self, service_name: ServiceName) -> Self::Output {
        ProvisionerFactory::new(self.provisioner_client.clone(), service_name)
    }
}

impl AbstractProvisionerFactory {
    pub fn new(provisioner_client: ProvisionerClient<Channel>) -> Self {
        Self { provisioner_client }
    }
}

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    service_name: ServiceName,
    provisioner_client: ProvisionerClient<Channel>,
    info: Option<DatabaseReadyInfo>,
    secrets: Option<BTreeMap<String, String>>,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<Channel>,
        service_name: ServiceName,
    ) -> Self {
        Self {
            provisioner_client,
            service_name,
            info: None,
            secrets: None,
        }
    }
}

#[async_trait]
impl Factory for ProvisionerFactory {
    async fn get_db_connection_string(
        &mut self,
        db_type: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        info!("Provisioning a {db_type} on the shuttle servers. This can take a while...");

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
        if let Some(ref secrets) = self.secrets {
            debug!("Returning previously fetched secrets");
            Ok(secrets.clone())
        } else {
            todo!()
        }
    }

    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }
}
