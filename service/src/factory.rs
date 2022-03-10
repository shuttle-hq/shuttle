use crate::Error;
use async_trait::async_trait;
use sqlx::PgPool;

#[async_trait]
pub trait Factory: Send + Sync {
    async fn get_sql_connection_string(&self) -> Result<String, crate::Error>;

    async fn get_postgres_connection_pool(&self) -> Result<PgPool, Error>;
}
