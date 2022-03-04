use crate::database::DatabaseResource;
use async_trait::async_trait;
use lib::ProjectConfig;
use sqlx::PgPool;
use std::convert::Into;
use std::sync::Arc;
use tokio::sync::Mutex;
use unveil_service::Factory;

pub(crate) struct UnveilFactory {
    database: Arc<Mutex<DatabaseResource>>,
    project: ProjectConfig,
}

impl UnveilFactory {
    pub(crate) fn new(database: Arc<Mutex<DatabaseResource>>, project: ProjectConfig) -> Self {
        Self { database, project }
    }
}

#[async_trait]
impl Factory for UnveilFactory {
    /// Lazily gets a connection pool
    async fn get_postgres_connection_pool(
        &mut self,
    ) -> Result<PgPool, unveil_service::Error> {
        self.database
            .lock()
            .await
            .get_client(&self.project.name)
            .await
            .map_err(Into::into)
    }
}
