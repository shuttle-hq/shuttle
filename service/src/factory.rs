use crate::Error;
use async_trait::async_trait;
use sqlx::PgPool;

#[async_trait]
pub trait Factory: Send + Sync {
    async fn get_sql_connection_string(&mut self) -> Result<String, crate::Error>;

    async fn get_postgres_connection_pool(&mut self) -> Result<PgPool, Error>;
}

#[async_trait]
impl Factory for Box<dyn Factory> {
    async fn get_sql_connection_string(&mut self) -> Result<String, crate::Error> {
        self.as_mut().get_sql_connection_string().await
    }

    async fn get_postgres_connection_pool(&mut self) -> Result<PgPool, Error> {
        self.as_mut().get_postgres_connection_pool().await
    }
}
