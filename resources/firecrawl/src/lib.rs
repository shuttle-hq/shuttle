use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{Error, IntoResource, ResourceFactory, ResourceInputBuilder};

use firecrawl::FirecrawlApp;

pub use firecrawl;

#[derive(Default, Serialize, Deserialize)]
pub struct Firecrawl {
    api_key: String,
    url: Option<String>,
}

impl Firecrawl {
    /// Name to give resource
    pub fn api_key(mut self, api_key: &str) -> Self {
        self.api_key = api_key.to_string();
        self
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }
}

#[async_trait]
impl ResourceInputBuilder for Firecrawl {
    type Input = Self;
    type Output = Self;

    async fn build(self, _factory: &ResourceFactory) -> Result<Self::Output, Error> {
        // factory can be used to get resources from Shuttle
        Ok(self)
    }
}

#[async_trait]
impl IntoResource<FirecrawlApp> for Firecrawl {
    async fn into_resource(self) -> Result<FirecrawlApp, Error> {
        let api_url = match self.url {
            Some(url) => url,
            None => "https://api.firecrawl.dev".to_string(),
        };

        let cfg = FirecrawlApp::new(Some(self.api_key), Some(api_url))
            .map_err(|x| shuttle_service::Error::Custom(x.into()))?;

        Ok(cfg)
    }
}
