#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use shuttle_service::{database, error::CustomError, Error, Factory, ResourceBuilder};
use tokio::runtime::Runtime;

pub struct ElastiCache;

#[async_trait]
impl ResourceBuilder<redis::aio::Connection> for ElastiCache {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        runtime: &Runtime,
    ) -> Result<redis::aio::Connection, Error> {
        let connection_string = factory
            .get_db_connection_string(database::Type::ElastiCache(
                database::ElastiCacheEngine::Redis,
            ))
            .await?;

        // A redis connection cannot cross runtime boundaries, so make sure to create the connection on the service end
        let conn = runtime
            .spawn(async move {
                let client = redis::Client::open(connection_string)
                    .expect("connection string should be valid");
                client.get_async_connection().await
            })
            .await
            .map_err(CustomError::new)?
            .map_err(CustomError::new)?;

        Ok(conn)
    }
}
