use crate::database::DatabaseResource;
use lib::ProjectConfig;
use std::sync::Arc;
use unveil_service::Factory;
use sqlx::pool::PoolConnection;
use async_trait::async_trait;

pub(crate) struct UnveilFactory {
    database: Arc<DatabaseResource>,
    project: ProjectConfig,
}

impl UnveilFactory {
    pub(crate) fn new(database: Arc<DatabaseResource>, project: ProjectConfig) -> Self {
        Self { database, project }
    }
}

#[async_trait]
impl Factory for UnveilFactory {
    /// Lazily gets a connection pool
    async fn get_postgres_connection_pool(
        &self,
        _name: &str,
    ) -> Result<PoolConnection<sqlx::Postgres>, unveil_service::Error> {
        self.database.get_client().await
    }
}
