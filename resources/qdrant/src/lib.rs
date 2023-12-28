use async_trait::async_trait;
use qdrant_client::prelude::*;
use serde::Serialize;
pub use shuttle_service::QdrantReadyInfo;
use shuttle_service::{Environment, Factory, QdrantInput, ResourceBuilder, Type};

/// A Qdrant vector database
#[derive(Serialize)]
pub struct Qdrant {
    pub cloud_url: String,
    pub api_key: String,
    pub local_url: Option<String>,
}

impl Qdrant {
    pub fn cloud_url(mut self, cloud_url: &str) -> Self {
        self.cloud_url = cloud_url.to_string();
        self
    }

    pub fn api_key(mut self, api_key: &str) -> Self {
        self.api_key = api_key.to_string();
        self
    }

    pub fn local_url(mut self, local_url: &str) -> Self {
        self.local_url = Some(local_url.to_string());
        self
    }
}

#[async_trait]
impl ResourceBuilder<QdrantClient> for Qdrant {
    const TYPE: Type = Type::Custom;

    type Config = Self;
    type Output = QdrantClientConfig;

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
        let md = factory.get_metadata();
        match md.env {
            Environment::Deployment => {
                if self.cloud_url.is_empty() {
                    Err(ShuttleError::Custom(CustomError::msg(
                        "missing `cloud_url`",
                    )))
                } else {
                    QdrantClientConfig::from_url(self.cloud_url).with_api_key(self.api_key)
                }
            }
            Environment::Local => match self.local_url {
                Some(ref local_url) => QdrantClientConfig::from_url(self.local_url),
                None => {
                    let url = factory.get_qdrant_connection(md.project_name).await?;
                    QdrantClientConfig::from_url(url)
                }
            },
        }
    }

    async fn build(client_config: &Self::Output) -> Result<QdrantClient, shuttle_service::Error> {
        // TODO: Handle error better?
        Ok(QdrantClient::new(Some(client_config))?)
    }
}
