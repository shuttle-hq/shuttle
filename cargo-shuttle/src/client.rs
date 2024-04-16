use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use headers::{Authorization, HeaderMapExt};
use percent_encoding::utf8_percent_encode;
use reqwest::header::HeaderMap;
use reqwest::{RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use shuttle_common::constants::headers::X_CARGO_SHUTTLE_VERSION;
use shuttle_common::models::deployment::DeploymentRequest;
use shuttle_common::models::{deployment, project, service, ToJson};
use shuttle_common::{resource, ApiKey, LogItem, VersionInfo};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::error;
use uuid::Uuid;

#[derive(Clone)]
pub struct ShuttleApiClient {
    client: reqwest::Client,
    api_url: String,
    api_key: Option<ApiKey>,
    /// alter behaviour to interact with the new platform
    beta: bool,
}

impl ShuttleApiClient {
    pub fn new(api_url: String, api_key: Option<ApiKey>, beta: bool) -> Self {
        Self {
            client: reqwest::Client::builder()
                .default_headers(
                    HeaderMap::try_from(&HashMap::from([(
                        X_CARGO_SHUTTLE_VERSION.clone(),
                        crate::VERSION.to_owned(),
                    )]))
                    .unwrap(),
                )
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap(),
            api_url,
            api_key,
            beta,
        }
    }

    pub fn set_api_key(&mut self, api_key: ApiKey) {
        self.api_key = Some(api_key);
    }

    fn set_auth_bearer(&self, builder: RequestBuilder) -> RequestBuilder {
        if let Some(ref api_key) = self.api_key {
            builder.bearer_auth(api_key.as_ref())
        } else {
            builder
        }
    }

    pub async fn get_api_versions(&self) -> Result<VersionInfo> {
        let url = format!("{}/versions", self.api_url);

        self.client
            .get(url)
            .send()
            .await?
            .json()
            .await
            .context("parsing API version info")
    }

    pub async fn check_project_name(&self, project_name: &str) -> Result<bool> {
        let url = format!("{}/projects/name/{project_name}", self.api_url);

        self.client
            .get(url)
            .send()
            .await
            .context("failed to check project name availability")?
            .to_json()
            .await
            .context("parsing name check response")
    }

    pub async fn deploy(
        &self,
        project: &str,
        deployment_req: DeploymentRequest,
    ) -> Result<deployment::Response> {
        let path = format!("/projects/{project}/services/{project}");
        let deployment_req = rmp_serde::to_vec(&deployment_req)
            .context("serialize DeploymentRequest as a MessagePack byte vector")?;

        let url = format!("{}{}", self.api_url, path);
        let mut builder = self.client.post(url);
        builder = self.set_auth_bearer(builder);

        builder
            .header("Transfer-Encoding", "chunked")
            .body(deployment_req)
            .send()
            .await
            .context("failed to send deployment to the Shuttle server")?
            .to_json()
            .await
    }

    pub async fn deploy_beta(
        &self,
        project: &str,
        deployment_req: DeploymentRequest,
    ) -> Result<deployment::EcsResponse> {
        let path = format!("/projects/{project}");
        let deployment_req = rmp_serde::to_vec(&deployment_req)
            .context("serialize DeploymentRequest as a MessagePack byte vector")?;

        let url = format!("{}{}", self.api_url, path);
        let mut builder = self.client.put(url);
        builder = self.set_auth_bearer(builder);

        builder
            .header("Transfer-Encoding", "chunked")
            .body(deployment_req)
            .send()
            .await
            .context("failed to send deployment to the Shuttle server")?
            .to_json()
            .await
    }

    pub async fn stop_service(&self, project: &str) -> Result<service::Summary> {
        let path = format!("/projects/{project}/services/{project}");

        self.delete(path).await
    }

    pub async fn get_service(&self, project: &str) -> Result<service::Summary> {
        let path = format!("/projects/{project}/services/{project}");

        self.get(path).await
    }

    pub async fn get_service_resources(&self, project: &str) -> Result<Vec<resource::Response>> {
        let path = format!("/projects/{project}/services/{project}/resources");

        self.get(path).await
    }

    pub async fn delete_service_resource(
        &self,
        project: &str,
        resource_type: &resource::Type,
    ) -> Result<()> {
        let path = format!(
            "/projects/{project}/services/{project}/resources/{}",
            utf8_percent_encode(
                &resource_type.to_string(),
                percent_encoding::NON_ALPHANUMERIC
            ),
        );

        self.delete(path).await
    }

    pub async fn create_project(
        &self,
        project: &str,
        config: &project::Config,
    ) -> Result<project::Response> {
        let path = format!("/projects/{project}");

        self.post(path, Some(config))
            .await
            .context("failed to make create project request")?
            .to_json()
            .await
    }

    pub async fn clean_project(&self, project: &str) -> Result<String> {
        let path = format!("/projects/{project}/clean");

        self.post(path, Option::<String>::None)
            .await
            .context("failed to get clean output")?
            .to_json()
            .await
    }

    pub async fn get_project(&self, project: &str) -> Result<project::Response> {
        let path = format!("/projects/{project}");

        self.get(path).await
    }

    pub async fn get_projects_list(&self) -> Result<Vec<project::Response>> {
        self.get("/projects".to_owned()).await
    }

    pub async fn stop_project(&self, project: &str) -> Result<project::Response> {
        let path = format!("/projects/{project}");

        self.delete(path).await
    }

    pub async fn delete_project(&self, project: &str) -> Result<String> {
        let path = if self.beta {
            format!("/projects/{project}")
        } else {
            format!("/projects/{project}/delete")
        };

        self.delete(path).await
    }

    pub async fn get_logs(&self, project: &str, deployment_id: &Uuid) -> Result<Vec<LogItem>> {
        let path = format!("/projects/{project}/deployments/{deployment_id}/logs");

        self.get(path)
            .await
            .context("Failed parsing logs. Is your cargo-shuttle outdated?")
    }

    pub async fn get_logs_ws(
        &self,
        project: &str,
        deployment_id: &Uuid,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let path = format!("/projects/{project}/ws/deployments/{deployment_id}/logs");

        self.ws_get(path).await
    }

    pub async fn get_deployments(
        &self,
        project: &str,
        page: u32,
        limit: u32,
    ) -> Result<Vec<deployment::Response>> {
        let path = format!(
            "/projects/{project}/deployments?page={}&limit={}",
            page.saturating_sub(1),
            limit,
        );

        self.get(path).await
    }

    pub async fn deployment_status(
        &self,
        project: &str,
        deployment_id: &str,
    ) -> Result<deployment::EcsResponse> {
        let path = format!("/projects/{project}/deployments/{deployment_id}");

        self.get(path).await
    }

    pub async fn get_deployment_details(
        &self,
        project: &str,
        deployment_id: &Uuid,
    ) -> Result<deployment::Response> {
        let path = format!("/projects/{project}/deployments/{deployment_id}");

        self.get(path).await
    }

    pub async fn reset_api_key(&self) -> Result<Response> {
        self.put("/users/reset-api-key".into(), Option::<()>::None)
            .await
    }

    async fn ws_get(&self, path: String) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let ws_url = self.api_url.clone().replace("http", "ws");
        let url = format!("{ws_url}{path}");
        let mut request = url.into_client_request()?;

        if let Some(ref api_key) = self.api_key {
            let auth_header = Authorization::bearer(api_key.as_ref())?;
            request.headers_mut().typed_insert(auth_header);
        }

        let (stream, _) = connect_async(request).await.with_context(|| {
            error!("failed to connect to websocket");
            "could not connect to websocket"
        })?;

        Ok(stream)
    }

    async fn get<M>(&self, path: String) -> Result<M>
    where
        M: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = self.client.get(url);
        builder = self.set_auth_bearer(builder);

        builder
            .send()
            .await
            .context("failed to make get request")?
            .to_json()
            .await
    }

    async fn post<T: Serialize>(&self, path: String, body: Option<T>) -> Result<Response> {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = self.client.post(url);
        builder = self.set_auth_bearer(builder);

        if let Some(body) = body {
            let body = serde_json::to_string(&body)?;
            builder = builder.body(body);
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.send().await?)
    }

    async fn put<T: Serialize>(&self, path: String, body: Option<T>) -> Result<Response> {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = self.client.put(url);
        builder = self.set_auth_bearer(builder);

        if let Some(body) = body {
            let body = serde_json::to_string(&body)?;
            builder = builder.body(body);
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.send().await?)
    }

    async fn delete<M>(&self, path: String) -> Result<M>
    where
        M: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = self.client.delete(url);
        builder = self.set_auth_bearer(builder);

        builder
            .send()
            .await
            .context("failed to make delete request")?
            .to_json()
            .await
    }
}
