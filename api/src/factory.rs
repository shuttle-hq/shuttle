use std::sync::Arc;
use unveil_service::Factory;

pub(crate) struct UnveilFactory {
    database: Arc<DatabaseState>,
    context: SomeContext
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
