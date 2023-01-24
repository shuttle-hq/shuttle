#![doc = include_str!("../README.md")]

use tokio::runtime::Runtime;

use async_trait::async_trait;
use shuttle_service::{database, error::CustomError, Error, Factory, ResourceBuilder};

pub struct ElastiCache;

#[async_trait]
impl ResourceBuilder<redis::Client> for ElastiCache {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        runtime: &Runtime,
    ) -> Result<redis::Client, Error> {
        todo!()
    }
}
