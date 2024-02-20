use async_trait::async_trait;
use qdrant_client::prelude::*;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    error::{CustomError, Error},
    resource::{ProvisionResourceRequest, Type},
    ContainerRequest, ContainerResponse, Environment, Factory, IntoResource, IntoResourceInput,
    ShuttleResourceOutput,
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

#[async_trait]
impl IntoResourceInput for Qdrant {
    type Input = ProvisionResourceRequest;
    // The response can be a provisioned container, depending on local/deployment and config.
    type Output = Wrapper;

    async fn into_resource_input(self, factory: &dyn Factory) -> Result<Self::Input, Error> {
        let md = factory.get_metadata();
        match md.env {
            Environment::Deployment => match self.cloud_url {
                Some(cloud_url) => Ok(ProvisionResourceRequest::new(
                    Type::Container,
                    serde_json::Value::Null, // deployment provisioner will ignore config for type==Container
                    serde_json::to_value(&QdrantClientConfigWrap {
                        url: cloud_url,
                        api_key: self.api_key,
                    })
                    .unwrap(),
                )),
                None => Err(Error::Custom(CustomError::msg(
                    "missing `cloud_url` parameter",
                ))),
            },
            Environment::Local => match self.local_url {
                Some(local_url) => Ok(ProvisionResourceRequest::new(
                    Type::Container,
                    serde_json::Value::Null, // local provisioner will ignore request if config is null
                    serde_json::to_value(&QdrantClientConfigWrap {
                        url: local_url,
                        api_key: self.api_key,
                    })
                    .unwrap(),
                )),
                None => Ok(ProvisionResourceRequest::new(
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
                )),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Wrapper(ShuttleResourceOutput<Option<ContainerResponse>>);

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
        let c = match self.0.output {
            Some(container) => QdrantClientConfigWrap {
                url: format!("http://localhost:{}", container.host_port),
                api_key: None,
            },
            None => serde_json::from_value(self.0.custom).map_err(|e| Error::Custom(e.into()))?,
        };
        Ok(QdrantClientConfig::from_url(&c.url)
            .with_api_key(c.api_key)
            .build()?)
    }
}
