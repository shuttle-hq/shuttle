use async_trait::async_trait;
use proto::provisioner::{provisioner_client::ProvisionerClient, DatabaseRequest};
use shuttle_common::{project::ProjectName, DatabaseReadyInfo};
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
    provisioner_address: String,
}

impl AbstractFactory for AbstractProvisionerFactory {
    type Output = ProvisionerFactory;

    fn get_factory(&self, project_name: ProjectName) -> Self::Output {
        ProvisionerFactory::new(
            self.provisioner_client.clone(),
            self.provisioner_address.clone(),
            project_name,
        )
    }
}

impl AbstractProvisionerFactory {
    pub fn new(
        provisioner_client: ProvisionerClient<Channel>,
        provisioner_address: String,
    ) -> Self {
        Self {
            provisioner_client,
            provisioner_address,
        }
    }
}

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    project_name: ProjectName,
    provisioner_client: ProvisionerClient<Channel>,
    provisioner_address: String,
    info: Option<DatabaseReadyInfo>,
}

impl ProvisionerFactory {
    pub(crate) fn new(
        provisioner_client: ProvisionerClient<Channel>,
        provisioner_address: String,
        project_name: ProjectName,
    ) -> Self {
        Self {
            provisioner_client,
            provisioner_address,
            project_name,
            info: None,
        }
    }
}

#[async_trait]
impl Factory for ProvisionerFactory {
    async fn get_sql_connection_string(&mut self) -> Result<String, shuttle_service::Error> {
        if let Some(ref info) = self.info {
            return Ok(info.connection_string(&self.provisioner_address));
        }

        let request = Request::new(DatabaseRequest {
            project_name: self.project_name.to_string(),
        });

        let response = self
            .provisioner_client
            .provision_database(request)
            .await
            .map_err(shuttle_service::error::CustomError::new)?
            .into_inner();

        let info: DatabaseReadyInfo = response.into();
        let conn_str = info.connection_string(&self.provisioner_address);
        self.info = Some(info);

        debug!("giving a sql connection string: {}", conn_str);
        Ok(conn_str)
    }
}
