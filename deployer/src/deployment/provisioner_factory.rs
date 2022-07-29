use async_trait::async_trait;
use shuttle_common::{database, project::ProjectName, DatabaseReadyInfo};
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_service::Factory;
use tonic::{transport::Channel, Request};
use tracing::debug;

/// Trait to make it easy to get a factory (service locator) for each service being started
pub trait AbstractFactory: Send + 'static {
    type Output: Factory;

    /// Get a factory for a specific project
    fn get_factory(&self, project_name: ProjectName) -> Self::Output;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractProvisionerFactory {
    provisioner_client: ProvisionerClient<Channel>,
}

impl AbstractFactory for AbstractProvisionerFactory {
    type Output = ProvisionerFactory;

    fn get_factory(&self, project_name: ProjectName) -> Self::Output {
        ProvisionerFactory::new(self.provisioner_client.clone(), project_name)
    }
}

impl AbstractProvisionerFactory {
    pub fn new(provisioner_client: ProvisionerClient<Channel>) -> Self {
        Self { provisioner_client }
    }
}

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    project_name: ProjectName,
    provisioner_client: ProvisionerClient<Channel>,
    info: Option<DatabaseReadyInfo>,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<Channel>,
        project_name: ProjectName,
    ) -> Self {
        Self {
            provisioner_client,
            project_name,
            info: None,
        }
    }
}

#[async_trait]
impl Factory for ProvisionerFactory {
    async fn get_sql_connection_string(
        &mut self,
        db_type: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        if let Some(ref info) = self.info {
            return Ok(info.connection_string_private());
        }

        let db_type: DbType = db_type.into();

        let request = Request::new(DatabaseRequest {
            project_name: self.project_name.to_string(),
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

        debug!("giving a sql connection string: {}", conn_str);
        Ok(conn_str)
    }
}
