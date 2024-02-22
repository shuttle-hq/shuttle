use async_trait::async_trait;
use qdrant_client::prelude::*;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    error::{CustomError, Error},
    resource::{ProvisionResourceRequest, Type},
    ContainerRequest, ContainerResponse, Environment, IntoResource, IntoResourceInput,
    ResourceFactory, ShuttleResourceOutput,
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

/// Conditionally request a Shuttle resource
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybeRequest {
    Request(ProvisionResourceRequest),
    NotRequest(QdrantClientConfigWrap),
}

#[async_trait]
impl IntoResourceInput for Qdrant {
    type Input = MaybeRequest;
    // The response can be a provisioned container, depending on local/deployment and config.
    type Output = Wrapper;

    async fn into_resource_input(self, factory: &ResourceFactory) -> Result<Self::Input, Error> {
        let md = factory.get_metadata();
        match md.env {
            Environment::Deployment => match self.cloud_url {
                Some(cloud_url) => Ok(MaybeRequest::NotRequest(
                    QdrantClientConfigWrap {
                        url: cloud_url,
                        api_key: self.api_key,
                    }),
                ),
                None => Err(Error::Custom(CustomError::msg(
                    "missing `cloud_url` parameter",
                ))),
            },
            Environment::Local => match self.local_url {
                Some(local_url) => Ok(MaybeRequest::NotRequest(
                    QdrantClientConfigWrap {
                        url: local_url,
                        api_key: self.api_key,
                    }),
                ),
                None => Ok(MaybeRequest::Request(
                    ProvisionResourceRequest::new(
                        Type::Container,
                        serde_json::to_value(&ContainerRequest {
                            project_name: md.project_name,
                            container_type: "qdrant".to_string(),
                            image: "docker.io/qdrant/qdrant:v1.7.4".to_string(),
                            port: "6334/tcp".to_string(),
                            env: vec![],
                        })
                        .unwrap(),
                        serde_json::Value::Null,
                    )
                )),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Wrapper {
    One(ShuttleResourceOutput<ContainerResponse>),
    Two(QdrantClientConfigWrap),
}

/// Scrappy wrapper over `QdrantClientConfig` to implement Clone and serde
/// for use in ResourceBuilder
#[derive(Clone, Serialize, Deserialize)]
pub struct QdrantClientConfigWrap {
    url: String,
    api_key: Option<String>,
}

#[async_trait]
impl IntoResource<QdrantClient> for Wrapper {
    async fn into_resource(self) -> Result<QdrantClient, Error> {
        let c = match self {
            Self::One(output) => {
                QdrantClientConfigWrap {
                    url: format!("http://localhost:{}", output.output.host_port),
                    api_key: None,
                }
            }
            Self::Two(i) => i,
        };
        Ok(QdrantClientConfig::from_url(&c.url)
            .with_api_key(c.api_key)
            .build()?)
    }
}
