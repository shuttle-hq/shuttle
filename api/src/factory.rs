use async_trait::async_trait;
use shuttle_common::DatabaseReadyInfo;
use shuttle_service::{database::Type, Factory};

use crate::database;

pub(crate) struct ShuttleFactory {
    database: database::State,
}

impl ShuttleFactory {
    pub(crate) fn new(database: database::State) -> Self {
        Self { database }
    }
}

impl ShuttleFactory {
    pub(crate) fn to_database_info(&self) -> Option<DatabaseReadyInfo> {
        self.database.to_info()
    }
}

#[async_trait]
impl Factory for ShuttleFactory {
    async fn get_sql_connection_string(
        &mut self,
        db_type: Type,
    ) -> Result<String, shuttle_service::Error> {
        let db_info = match db_type {
            Type::Shared => self
                .database
                .request()
                .await
                .map_err(shuttle_service::error::CustomError::new)?,
            Type::AwsRds(engine) => self.database.aws_rds(engine).await?,
        };

        let conn_str = db_info.connection_string_private();

        debug!("giving a sql connection string: {}", conn_str);
        Ok(conn_str)
    }
}
