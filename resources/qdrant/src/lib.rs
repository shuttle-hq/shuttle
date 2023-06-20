use async_trait::async_trait;
use qdrant_client::prelude::*;
use serde::Serialize;
pub use shuttle_service::QdrantReadyInfo;
use shuttle_service::{Factory, QdrantInput, ResourceBuilder, Type};

#[derive(Serialize)]
#[doc = "A resource connected to a Qdrant instance"]
pub struct Qdrant {
    config: QdrantInput
}

impl Qdrant {
    pub fn cloud_url(mut self, cloud_url: &str) -> Self {
        self.config.cloud_url = Some(cloud_url.to_string());

        self
    }

    pub fn api_key(mut self, api_key: &str) -> Self {
        self.config.api_key = Some(api_key.to_string());

        self
    }
}

#[doc = "Gets a connection to Qdrant"]
#[async_trait]
impl ResourceBuilder<QdrantClient> for Qdrant {
    const TYPE: Type = Type::Qdrant;

    type Config = QdrantInput;
    type Output = QdrantReadyInfo;

    fn new() -> Self {
        Self {
            config: Default::default(),
        }
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        assert!(self.config.cloud_url.is_some(), "`cloud_url` must be set");
        assert!(self.config.api_key.is_some(), "`api_key` must be set");

        factory.get_qdrant_connection(self.config.cloud_url.unwrap(), self.config.api_key.unwrap()).await
    }

    async fn build(build_data: &Self::Output) -> Result<QdrantClient, shuttle_service::Error> {
        let mut config = QdrantClientConfig::from_url(&build_data.url);

        if let Some(api_key) = &build_data.api_key {
            config.set_api_key(api_key);
        }

        Ok(QdrantClient::new(Some(config))?)
    }
}
