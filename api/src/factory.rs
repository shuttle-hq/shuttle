use std::sync::Arc;

use crate::database;
use async_trait::async_trait;
use lib::project::ProjectConfig;
use tokio::sync::RwLock;
use unveil_service::Factory;

pub(crate) struct UnveilFactory<'a> {
    database: Arc<RwLock<&'a mut database::State>>,
    project: ProjectConfig,
    ctx: database::Context,
}

impl<'a> UnveilFactory<'a> {
    pub(crate) fn new(
        database: &'a mut database::State,
        project: ProjectConfig,
        ctx: database::Context,
    ) -> Self {
        Self {
            database: Arc::new(RwLock::new(database)),
            project,
            ctx,
        }
    }
}

#[async_trait]
impl Factory for UnveilFactory<'_> {
    async fn get_sql_connection_string(&self) -> Result<String, unveil_service::Error> {
        let ready_state = self
            .database
            .write()
            .await
            .advance(&self.project.name(), &self.ctx)
            .await
            .map_err(unveil_service::Error::from)?;

        Ok(ready_state.connection_string("localhost"))
    }
}
