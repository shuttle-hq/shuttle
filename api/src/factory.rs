use std::sync::Arc;
use lib::ProjectConfig;
use unveil_service::Factory;
use crate::database::DatabaseState;

pub(crate) struct UnveilFactory {
    database: Arc<DatabaseState>,
    project: ProjectConfig
}

impl UnveilFactory {
    pub(crate) fn new(database: Arc<DatabaseState>, project: ProjectConfig) -> Self {
        Self {
            database,
            project
        }
    }
}

impl Factory for UnveilFactory {
    /// Lazily gets a connection pool
    fn get_postgres_connection_pool(
        &self,
        _name: &str,
    ) -> Result<sqlx::PgPool, unveil_service::Error> {
        self.database.get_client()
    }
}
