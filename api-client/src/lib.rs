use std::time::Duration;

use anyhow::{Context, Result};
use headers::{Authorization, HeaderMapExt};
use percent_encoding::utf8_percent_encode;
use reqwest::header::HeaderMap;
use reqwest::Response;
use reqwest_middleware::{ClientWithMiddleware, RequestBuilder};
use serde::{Deserialize, Serialize};
use shuttle_common::models::{
    certificate::{
        AddCertificateRequest, CertificateListResponse, CertificateResponse,
        DeleteCertificateRequest,
    },
    deployment::{
        DeploymentListResponse, DeploymentRequest, DeploymentResponse, UploadArchiveResponse,
    },
    log::LogsResponse,
    project::{ProjectCreateRequest, ProjectListResponse, ProjectResponse, ProjectUpdateRequest},
    resource::{ProvisionResourceRequest, ResourceListResponse, ResourceResponse, ResourceType},
    team::TeamListResponse,
    user::UserResponse,
};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[cfg(feature = "tracing")]
mod middleware;
#[cfg(feature = "tracing")]
use crate::middleware::LoggingMiddleware;
#[cfg(feature = "tracing")]
use tracing::{debug, error};

mod util;
use util::ToJson;

#[derive(Clone)]
pub struct ShuttleApiClient {
    pub client: ClientWithMiddleware,
    pub api_url: String,
    pub api_key: Option<String>,
}

impl ShuttleApiClient {
    pub fn new(
        api_url: String,
        api_key: Option<String>,
        headers: Option<HeaderMap>,
        timeout: Option<u64>,
    ) -> Self {
        let mut builder = reqwest::Client::builder();
        if let Some(h) = headers {
            builder = builder.default_headers(h);
        }
        let client = builder
            .timeout(Duration::from_secs(timeout.unwrap_or(60)))
            .build()
            .unwrap();

        let builder = reqwest_middleware::ClientBuilder::new(client);
        #[cfg(feature = "tracing")]
        let builder = builder.with(LoggingMiddleware);
        let client = builder.build();

        Self {
            client,
            api_url,
            api_key,
        }
    }

    pub fn set_auth_bearer(&self, builder: RequestBuilder) -> RequestBuilder {
        if let Some(ref api_key) = self.api_key {
            builder.bearer_auth(api_key)
        } else {
            builder
        }
    }

    pub async fn get_device_auth_ws(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        self.ws_get("/device-auth/ws")
            .await
            .with_context(|| "failed to connect to auth endpoint")
    }

    pub async fn check_project_name(&self, project_name: &str) -> Result<bool> {
        let url = format!("{}/projects/{project_name}/name", self.api_url);

        self.client
            .get(url)
            .send()
            .await
            .context("failed to check project name availability")?
            .to_json()
            .await
            .context("parsing name check response")
    }

    pub async fn get_current_user(&self) -> Result<UserResponse> {
        self.get_json("/users/me").await
    }

    pub async fn deploy(
        &self,
        project: &str,
        deployment_req: DeploymentRequest,
    ) -> Result<DeploymentResponse> {
        let path = format!("/projects/{project}/deployments");
        self.post_json(path, Some(deployment_req)).await
    }

    pub async fn upload_archive(
        &self,
        project: &str,
        data: Vec<u8>,
    ) -> Result<UploadArchiveResponse> {
        let path = format!("/projects/{project}/archives");

        let url = format!("{}{}", self.api_url, path);
        let mut builder = self.client.post(url);
        builder = self.set_auth_bearer(builder);

        builder
            .body(data)
            .send()
            .await
            .context("failed to upload archive")?
            .to_json()
            .await
    }

    pub async fn redeploy(&self, project: &str, deployment_id: &str) -> Result<DeploymentResponse> {
        let path = format!("/projects/{project}/deployments/{deployment_id}/redeploy");

        self.post_json(path, Option::<()>::None).await
    }

    pub async fn stop_service(&self, project: &str) -> Result<String> {
        let path = format!("/projects/{project}/deployments");

        self.delete_json(path).await
    }

    pub async fn get_service_resources(&self, project: &str) -> Result<ResourceListResponse> {
        self.get_json(format!("/projects/{project}/resources"))
            .await
    }

    async fn _dump_service_resource(
        &self,
        project: &str,
        resource_type: &ResourceType,
    ) -> Result<Vec<u8>> {
        let r#type = resource_type.to_string();
        let r#type = utf8_percent_encode(&r#type, percent_encoding::NON_ALPHANUMERIC).to_owned();

        let res = self
            .get(
                format!(
                    "/projects/{project}/services/{project}/resources/{}/dump",
                    r#type
                ),
                Option::<()>::None,
            )
            .await?;

        let bytes = res.bytes().await?;

        Ok(bytes.to_vec())
    }

    pub async fn delete_service_resource(
        &self,
        project: &str,
        resource_type: &ResourceType,
    ) -> Result<String> {
        let r#type = resource_type.to_string();
        let r#type = utf8_percent_encode(&r#type, percent_encoding::NON_ALPHANUMERIC).to_owned();

        self.delete_json(format!("/projects/{project}/resources/{}", r#type))
            .await
    }
    pub async fn provision_resource(
        &self,
        project: &str,
        req: ProvisionResourceRequest,
    ) -> Result<ResourceResponse> {
        self.post_json(format!("/projects/{project}/resources"), Some(req))
            .await
    }
    pub async fn get_secrets(&self, project: &str) -> Result<ResourceResponse> {
        self.get_json(format!("/projects/{project}/resources/secrets"))
            .await
    }

    pub async fn list_certificates(&self, project: &str) -> Result<CertificateListResponse> {
        self.get_json(format!("/projects/{project}/certificates"))
            .await
    }
    pub async fn add_certificate(
        &self,
        project: &str,
        subject: String,
    ) -> Result<CertificateResponse> {
        self.post_json(
            format!("/projects/{project}/certificates"),
            Some(AddCertificateRequest { subject }),
        )
        .await
    }
    pub async fn delete_certificate(&self, project: &str, subject: String) -> Result<String> {
        self.delete_json_with_body(
            format!("/projects/{project}/certificates"),
            DeleteCertificateRequest { subject },
        )
        .await
    }

    pub async fn create_project(&self, name: &str) -> Result<ProjectResponse> {
        self.post_json(
            "/projects",
            Some(ProjectCreateRequest {
                name: name.to_string(),
            }),
        )
        .await
    }

    pub async fn get_project(&self, project: &str) -> Result<ProjectResponse> {
        self.get_json(format!("/projects/{project}")).await
    }

    pub async fn get_projects_list(&self) -> Result<ProjectListResponse> {
        self.get_json("/projects".to_owned()).await
    }

    pub async fn update_project(
        &self,
        project: &str,
        req: ProjectUpdateRequest,
    ) -> Result<ProjectResponse> {
        self.put_json(format!("/projects/{project}"), Some(req))
            .await
    }

    pub async fn delete_project(&self, project: &str) -> Result<String> {
        self.delete_json(format!("/projects/{project}")).await
    }

    #[allow(unused)]
    async fn get_teams_list(&self) -> Result<TeamListResponse> {
        self.get_json("/teams").await
    }

    pub async fn get_deployment_logs(
        &self,
        project: &str,
        deployment_id: &str,
    ) -> Result<LogsResponse> {
        let path = format!("/projects/{project}/deployments/{deployment_id}/logs");

        self.get_json(path).await
    }

    pub async fn get_deployments(
        &self,
        project: &str,
        page: i32,
        per_page: i32,
    ) -> Result<DeploymentListResponse> {
        let path = format!(
            "/projects/{project}/deployments?page={}&per_page={}",
            page.saturating_sub(1).max(0),
            per_page.max(1),
        );

        self.get_json(path).await
    }
    pub async fn get_current_deployment(
        &self,
        project: &str,
    ) -> Result<Option<DeploymentResponse>> {
        let path = format!("/projects/{project}/deployments/current");

        self.get_json(path).await
    }

    pub async fn get_deployment(
        &self,
        project: &str,
        deployment_id: &str,
    ) -> Result<DeploymentResponse> {
        let path = format!("/projects/{project}/deployments/{deployment_id}");

        self.get_json(path).await
    }

    pub async fn reset_api_key(&self) -> Result<Response> {
        self.put("/users/reset-api-key", Option::<()>::None).await
    }

    pub async fn ws_get(
        &self,
        path: impl AsRef<str>,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let ws_url = self.api_url.clone().replace("http", "ws");
        let url = format!("{ws_url}{}", path.as_ref());
        let mut req = url.into_client_request()?;

        #[cfg(feature = "tracing")]
        debug!("WS Request: {} {}", req.method(), req.uri());

        if let Some(ref api_key) = self.api_key {
            let auth_header = Authorization::bearer(api_key.as_ref())?;
            req.headers_mut().typed_insert(auth_header);
        }

        let (stream, _) = connect_async(req).await.with_context(|| {
            #[cfg(feature = "tracing")]
            error!("failed to connect to websocket");
            "could not connect to websocket"
        })?;

        Ok(stream)
    }

    pub async fn get<T: Serialize>(
        &self,
        path: impl AsRef<str>,
        body: Option<T>,
    ) -> Result<Response> {
        let url = format!("{}{}", self.api_url, path.as_ref());

        let mut builder = self.client.get(url);
        builder = self.set_auth_bearer(builder);

        if let Some(body) = body {
            let body = serde_json::to_string(&body)?;
            #[cfg(feature = "tracing")]
            debug!("Outgoing body: {}", body);
            builder = builder.body(body);
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.send().await?)
    }

    pub async fn get_json<R>(&self, path: impl AsRef<str>) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.get(path, Option::<()>::None).await?.to_json().await
    }

    pub async fn get_json_with_body<R, T: Serialize>(
        &self,
        path: impl AsRef<str>,
        body: T,
    ) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.get(path, Some(body)).await?.to_json().await
    }

    pub async fn post<T: Serialize>(
        &self,
        path: impl AsRef<str>,
        body: Option<T>,
    ) -> Result<Response> {
        let url = format!("{}{}", self.api_url, path.as_ref());

        let mut builder = self.client.post(url);
        builder = self.set_auth_bearer(builder);

        if let Some(body) = body {
            let body = serde_json::to_string(&body)?;
            #[cfg(feature = "tracing")]
            debug!("Outgoing body: {}", body);
            builder = builder.body(body);
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.send().await?)
    }

    pub async fn post_json<T: Serialize, R>(
        &self,
        path: impl AsRef<str>,
        body: Option<T>,
    ) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.post(path, body).await?.to_json().await
    }

    pub async fn put<T: Serialize>(
        &self,
        path: impl AsRef<str>,
        body: Option<T>,
    ) -> Result<Response> {
        let url = format!("{}{}", self.api_url, path.as_ref());

        let mut builder = self.client.put(url);
        builder = self.set_auth_bearer(builder);

        if let Some(body) = body {
            let body = serde_json::to_string(&body)?;
            #[cfg(feature = "tracing")]
            debug!("Outgoing body: {}", body);
            builder = builder.body(body);
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.send().await?)
    }

    pub async fn put_json<T: Serialize, R>(
        &self,
        path: impl AsRef<str>,
        body: Option<T>,
    ) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.put(path, body).await?.to_json().await
    }

    pub async fn delete<T: Serialize>(
        &self,
        path: impl AsRef<str>,
        body: Option<T>,
    ) -> Result<Response> {
        let url = format!("{}{}", self.api_url, path.as_ref());

        let mut builder = self.client.delete(url);
        builder = self.set_auth_bearer(builder);

        if let Some(body) = body {
            let body = serde_json::to_string(&body)?;
            #[cfg(feature = "tracing")]
            debug!("Outgoing body: {}", body);
            builder = builder.body(body);
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.send().await?)
    }

    pub async fn delete_json<R>(&self, path: impl AsRef<str>) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.delete(path, Option::<()>::None).await?.to_json().await
    }

    pub async fn delete_json_with_body<R, T: Serialize>(
        &self,
        path: impl AsRef<str>,
        body: T,
    ) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.delete(path, Some(body)).await?.to_json().await
    }
}
