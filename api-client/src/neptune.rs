use anyhow::Result;
use bytes::Bytes;
use impulse_common::types::project::{CreateDeploymentRequest, CreateProjectRequest, ProjectKind};
use impulse_common::types::ProjectSpec;
use impulse_common::types::ProjectStatusResponse;

use crate::{
    neptune_types::{
        CheckCompatibilityRequest, CompatibilityReport, GenerateRequest, GenerateResponse,
        PlatformSpecDoc,
    },
    util::{ParsedJson, ToBodyContent},
    ShuttleApiClient,
};

#[derive(Clone)]
pub struct NeptuneClient {
    pub api_client: ShuttleApiClient,
    pub ai_service_client: ShuttleApiClient,
}

impl NeptuneClient {
    pub async fn get_agents_md(&self) -> Result<String> {
        self.ai_service_client
            .get("/v1/agents.md", Option::<()>::None)
            .await?
            .to_text()
            .await
    }

    pub async fn get_projects(&self) -> Result<ParsedJson<Vec<ProjectStatusResponse>>> {
        self.api_client
            .get("/projects", Option::<()>::None)
            .await?
            .to_json()
            .await
    }

    pub async fn get_project_by_id(&self, id: &str) -> Result<ParsedJson<ProjectStatusResponse>> {
        self.api_client
            .get(format!("/projects/{id}"), Option::<()>::None)
            .await?
            .to_json()
            .await
    }

    pub async fn delete_project_by_id(&self, id: &str) -> Result<ParsedJson<()>> {
        self.api_client
            .delete(format!("/projects/{id}"), Option::<()>::None)
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

    pub async fn get_project_id_from_name(&self, name: &str) -> Result<Option<String>> {
        let projects = self.get_projects().await?.into_inner();
        Ok(projects.into_iter().find(|p| p.name == name).map(|p| p.id))
    }

    pub async fn create_project(
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

    pub async fn create_deployment(
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

    pub async fn generate_spec(
        &self,
        payload: Vec<u8>,
        project_name: &str,
    ) -> Result<PlatformSpecDoc> {
        let url = format!("{}/v1/generate/spec", self.ai_service_client.api_url);

        let mut builder = self.ai_service_client.client.post(url);
        builder = self.ai_service_client.set_auth_bearer(builder);
        let form = GenerateRequest::from((payload, project_name)).into_multipart()?;

        builder = builder.multipart(form);

        let res = builder.send().await?;
        match res.error_for_status_ref() {
            Ok(_) => Ok(res.json::<PlatformSpecDoc>().await?),
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

    pub async fn generate(&self, payload: Vec<u8>, project_name: &str) -> Result<GenerateResponse> {
        let url = format!("{}/v1/generate", self.ai_service_client.api_url);

        let mut builder = self.ai_service_client.client.post(url);
        builder = self.ai_service_client.set_auth_bearer(builder);

        let form = GenerateRequest::from((payload, project_name)).into_multipart()?;

        builder = builder.multipart(form);

        let res = builder.send().await?;
        match res.error_for_status_ref() {
            Ok(_) => Ok(res.json::<GenerateResponse>().await?),
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

    pub async fn check_compatibility(&self, payload: Vec<u8>) -> Result<CompatibilityReport> {
        let url = format!("{}/v1/check/compatibility", self.ai_service_client.api_url);

        let mut builder = self.ai_service_client.client.post(url);
        builder = self.ai_service_client.set_auth_bearer(builder);

        let form = CheckCompatibilityRequest::from(payload).into_multipart()?;

        let res = builder.multipart(form).send().await?;

        match res.error_for_status_ref() {
            Ok(_) => Ok(res.json::<CompatibilityReport>().await?),
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
