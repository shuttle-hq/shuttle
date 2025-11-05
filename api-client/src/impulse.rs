use anyhow::Result;
use bytes::Bytes;
use impulse_common::types::ProjectStatusResponse;

use crate::{
    util::{ParsedJson, ToBodyContent},
    ShuttleApiClient,
};

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

    pub async fn get_impulse_projects(&self) -> Result<ParsedJson<Vec<ProjectStatusResponse>>> {
        self.api_client
            .get("/projects", Option::<()>::None)
            .await?
            .to_json()
            .await
    }

    pub async fn get_impulse_project(&self, id: &str) -> Result<ParsedJson<ProjectStatusResponse>> {
        self.api_client
            .get(format!("/project/{id}"), Option::<()>::None)
            .await?
            .to_json()
            .await
    }

    pub async fn registry_auth(&self) -> std::result::Result<Bytes, anyhow::Error> {
        self.api_client
            .post("/registry_auth", Option::<()>::None)
            .await?
            .bytes()
            .await
            .map_err(Into::into)
    }
}
