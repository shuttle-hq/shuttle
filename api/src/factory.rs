use std::sync::Arc;

use crate::database;
use async_trait::async_trait;
use lib::project::ProjectConfig;
use tokio::sync::RwLock;
use unveil_service::Factory;

pub(crate) struct UnveilFactory<'a> {
    database: &'a mut database::State
}

impl<'a> UnveilFactory<'a> {
    pub(crate) fn new(
        database: &'a mut database::State
    ) -> Self {
        Self {
            database
        }
    }
}

#[async_trait]
impl Factory for UnveilFactory<'_> {
    async fn get_sql_connection_string(&mut self) -> Result<String, unveil_service::Error> {
        let conn_str = self
            .database
            .request()
            .connection_string("localhost");
        debug!("giving a sql connection string: {}", conn_str);
        Ok(conn_str)
    }
}
