use async_trait::async_trait;
use shuttle_proto::provisioner::{
    database_request::DbType, provisioner_client::ProvisionerClient, DatabaseRequest,
};
use shuttle_common::{project::ProjectName, DatabaseReadyInfo};
use shuttle_service::{database::Type, Factory};
use tonic::{transport::Channel, Request};

pub(crate) struct ShuttleFactory {
    project_name: ProjectName,
    provisioner_client: ProvisionerClient<Channel>,
    info: Option<DatabaseReadyInfo>,
}

impl ShuttleFactory {
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

    pub(crate) fn into_database_info(self) -> Option<DatabaseReadyInfo> {
        self.info
    }
}

#[async_trait]
impl Factory for ShuttleFactory {
    async fn get_sql_connection_string(
        &mut self,
        db_type: Type,
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
