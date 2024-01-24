use async_trait::async_trait;
use qdrant_client::prelude::*;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    error::{CustomError, Error as ShuttleError},
    resource::Type,
    ContainerRequest, Environment, Factory, IntoResource, ResourceBuilder,
};

/// A Qdrant vector database
#[derive(Default, Serialize)]
pub struct Qdrant {
    /// Required if deploying
    cloud_url: Option<String>,
    /// Required if url endpoint is protected by key
    api_key: Option<String>,
    /// If given, use this instead of the default docker container on local run
    local_url: Option<String>,
}

/// Scrappy wrapper over `QdrantClientConfig` to implement Clone and serde
/// for use in ResourceBuilder
#[derive(Clone, Serialize, Deserialize)]
pub struct QdrantClientConfigWrap {
    url: String,
    api_key: Option<String>,
}

impl Qdrant {
    pub fn cloud_url(mut self, cloud_url: &str) -> Self {
        self.cloud_url = Some(cloud_url.to_string());
        self
    }
    pub fn api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }
    pub fn local_url(mut self, local_url: &str) -> Self {
        self.local_url = Some(local_url.to_string());
        self
    }
}

#[async_trait]
impl ResourceBuilder for Qdrant {
    const TYPE: Type = Type::Custom;

    type Config = Self;
    type Output = QdrantClientConfigWrap;

    fn config(&self) -> &Self::Config {
        self
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        let md = factory.get_metadata();
        match md.env {
            Environment::Deployment => match self.cloud_url {
                Some(cloud_url) => Ok(QdrantClientConfigWrap {
                    url: cloud_url,
                    api_key: self.api_key,
                }),
                None => Err(ShuttleError::Custom(CustomError::msg(
                    "missing `cloud_url` parameter",
                ))),
            },
            Environment::Local => match self.local_url {
                Some(local_url) => Ok(QdrantClientConfigWrap {
                    url: local_url,
                    api_key: self.api_key,
                }),
                None => {
                    let port = factory
                        .get_container(ContainerRequest {
                            project_name: md.project_name,
                            container_type: "qdrant".to_string(),
                            image: "docker.io/qdrant/qdrant:latest".to_string(),
                            port: "6334/tcp".to_string(),
                            env: vec![],
                        })
                        .await?
                        .host_port;
                    let url = format!("http://localhost:{port}");

                    Ok(QdrantClientConfigWrap { url, api_key: None })
                }
            },
        }
    }
}

#[async_trait]
impl IntoResource<QdrantClient> for QdrantClientConfigWrap {
    async fn into_resource(self) -> Result<QdrantClient, shuttle_service::Error> {
        Ok(QdrantClientConfig::from_url(&self.url)
            .with_api_key(self.api_key)
            .build()?)
    }
}
