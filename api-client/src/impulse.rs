use anyhow::Result;

use crate::{util::ToBodyContent, ShuttleApiClient};

#[derive(Clone)]
pub struct ImpulseClient {
    pub api_client: ShuttleApiClient,
    pub ai_service_client: ShuttleApiClient,
}

impl ImpulseClient {
    pub async fn get_agents_md(&self) -> Result<String> {
        self.ai_service_client
            .get("/v1/agents.md", Option::<()>::None)
            .await?
            .to_text()
            .await
    }
}
