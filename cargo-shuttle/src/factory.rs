use anyhow::anyhow;
use async_trait::async_trait;
use shuttle_service::Factory;

pub struct LocalFactory {}

#[async_trait]
impl Factory for LocalFactory {
    async fn get_sql_connection_string(&mut self) -> Result<String, shuttle_service::Error> {
        Err(shuttle_service::Error::Custom(anyhow!(
            "Database dependencies are not supported for local runs yet. Try deploying instead",
        )))
    }
}
