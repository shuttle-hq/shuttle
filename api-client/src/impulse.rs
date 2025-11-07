use anyhow::Result;
use bytes::Bytes;
use impulse_common::types::project::{CreateDeploymentRequest, CreateProjectRequest, ProjectKind};
use impulse_common::types::ProjectSpec;
use impulse_common::types::ProjectStatusResponse;
use reqwest::multipart::Form;
use reqwest::multipart::Part;

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

    pub async fn get_impulse_project_by_id(
        &self,
        id: &str,
    ) -> Result<ParsedJson<ProjectStatusResponse>> {
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

    pub async fn get_impulse_project_id_from_name(&self, name: &str) -> Result<Option<String>> {
        let projects = self.get_impulse_projects().await?.into_inner();
        Ok(projects.into_iter().find(|p| p.name == name).map(|p| p.id))
    }

    pub async fn create_impulse_project(
        &self,
        spec: &ProjectSpec,
    ) -> Result<ParsedJson<ProjectStatusResponse>> {
        let payload = CreateProjectRequest {
            name: String::from(&spec.name),
            kind: ProjectKind::clone(&spec.kind),
        };
        self.api_client
            .post("/projects", Some(payload))
            .await?
            .to_json()
            .await
    }

    pub async fn create_impulse_deployment(
        &self,
        spec: &ProjectSpec,
        id: &str,
        image: &str,
    ) -> Result<ParsedJson<ProjectStatusResponse>> {
        let mut extra = serde_json::Map::with_capacity(2);
        extra.insert(
            String::from("name"),
            serde_json::Value::String(String::from(&spec.name)),
        );
        extra.insert(
            String::from("image"),
            serde_json::Value::String(String::from(image)),
        );
        let payload = CreateDeploymentRequest {
            kind: ProjectKind::clone(&spec.kind),
            resources: Vec::clone(&spec.resources),
            extra,
        };
        self.api_client
            .post(format!("/projects/{id}/deployments"), Some(payload))
            .await?
            .to_json()
            .await
    }

    pub async fn generate_impulse_spec(&self, payload: Vec<u8>) -> Result<Bytes> {
        let url = format!("{}/v1/generate/spec", self.ai_service_client.api_url);

        let mut builder = self.ai_service_client.client.post(url);
        builder = self.ai_service_client.set_auth_bearer(builder);
        let form = Form::new()
            .part(
                "project",
                Part::bytes(payload)
                    .file_name("project.zip")
                    .mime_str("application/octet-stream")?,
            )
            .text("project_name", "hickyblue");

        builder = builder.multipart(form);
        builder = builder.header("Accept", "application/json");
        builder = builder.header("Content-Type", "multipart/from-data");

        let res = builder.send().await?;
        match res.error_for_status_ref() {
            Ok(_) => Ok(res.bytes().await?),
            Err(e) => {
                tracing::error!(
                    "{:?}: {:?}",
                    e,
                    str::from_utf8(&res.bytes().await?.to_vec())?
                );
                Err(e.into())
            }
        }
    }
}
