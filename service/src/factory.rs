use async_trait::async_trait;

#[async_trait]
pub trait Factory: Send + Sync {
    async fn get_sql_connection_string(&mut self) -> Result<String, crate::Error>;
}
