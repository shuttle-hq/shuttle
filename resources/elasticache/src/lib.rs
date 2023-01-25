#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use shuttle_service::{database, error::CustomError, Error, Factory, ResourceBuilder, Runtime};

pub struct Redis;

#[async_trait]
impl ResourceBuilder<redis::Client> for Redis {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        _runtime: &Runtime,
    ) -> Result<redis::Client, Error> {
        let connection_string = factory
            .get_db_connection_string(database::Type::ElastiCache(
                database::ElastiCacheEngine::Redis,
            ))
            .await?;

        let client = redis::Client::open(connection_string).map_err(CustomError::new)?;

        Ok(client)
    }
}
