use async_openai::config::OpenAIConfig;
use async_openai::Client;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{CustomError, Error, IntoResource, ResourceFactory, ResourceInputBuilder};

pub use async_openai;

#[derive(Default, Serialize)]
pub struct OpenAI {
    api_base: Option<String>,
    api_key: Option<String>,
    org_id: Option<String>,
    project_id: Option<String>,
}

impl OpenAI {
    pub fn api_base(mut self, api_base: &str) -> Self {
        self.api_base = Some(api_base.to_string());
        self
    }
    pub fn api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }
    pub fn org_id(mut self, org_id: &str) -> Self {
        self.org_id = Some(org_id.to_string());
        self
    }
    pub fn project_id(mut self, project_id: &str) -> Self {
        self.project_id = Some(project_id.to_string());
        self
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    api_base: Option<String>,
    api_key: String,
    org_id: Option<String>,
    project_id: Option<String>,
}

#[async_trait]
impl ResourceInputBuilder for OpenAI {
    type Input = Config;
    type Output = Config;

    async fn build(self, _factory: &ResourceFactory) -> Result<Self::Input, Error> {
        let api_key = self
            .api_key
            .ok_or(Error::Custom(CustomError::msg("Open AI API key required")))?;
        let config = Config {
            api_base: self.api_base,
            api_key,
            org_id: self.org_id,
            project_id: self.project_id,
        };
        Ok(config)
    }
}

#[async_trait]
impl IntoResource<Client<OpenAIConfig>> for Config {
    async fn into_resource(self) -> Result<Client<OpenAIConfig>, Error> {
        let mut openai_config = OpenAIConfig::new().with_api_key(self.api_key);
        if let Some(api_base) = self.api_base {
            openai_config = openai_config.with_api_base(api_base)
        }
        if let Some(org_id) = self.org_id {
            openai_config = openai_config.with_org_id(org_id)
        }
        if let Some(project_id) = self.project_id {
            openai_config = openai_config.with_project_id(project_id)
        }
        Ok(Client::with_config(openai_config))
    }
}
