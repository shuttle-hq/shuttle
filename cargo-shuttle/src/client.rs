use std::fmt::Write;

use anyhow::{Context, Result};
use headers::{Authorization, HeaderMapExt};
use reqwest::{Body, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::Deserialize;
use shuttle_common::models::{deployment, project, secret, service, ToJson};
use shuttle_common::project::ProjectName;
use shuttle_common::{ApiKey, ApiUrl, LogItem};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::error;
use uuid::Uuid;

pub struct Client {
    api_url: ApiUrl,
    api_key: Option<ApiKey>,
}

impl Client {
    pub fn new(api_url: ApiUrl) -> Self {
        Self {
            api_url,
            api_key: None,
        }
    }

    pub fn set_api_key(&mut self, api_key: ApiKey) {
        self.api_key = Some(api_key);
    }

    pub async fn deploy(
        &self,
        data: Vec<u8>,
        project: &ProjectName,
        no_test: bool,
    ) -> Result<deployment::Response> {
        let mut path = format!(
            "/projects/{}/services/{}",
            project.as_str(),
            project.as_str()
        );

        if no_test {
            let _ = write!(path, "?no-test");
        }

        self.post(path, Some(data))
            .await
            .context("failed to send deployment to the Shuttle server")?
            .to_json()
            .await
    }

    pub async fn delete_service(&self, project: &ProjectName) -> Result<service::Detailed> {
        let path = format!(
            "/projects/{}/services/{}",
            project.as_str(),
            project.as_str()
        );

        self.delete(path).await
    }

    pub async fn get_service_details(&self, project: &ProjectName) -> Result<service::Detailed> {
        let path = format!(
            "/projects/{}/services/{}",
            project.as_str(),
            project.as_str()
        );

        self.get(path).await
    }

    pub async fn get_service_summary(&self, project: &ProjectName) -> Result<service::Summary> {
        let path = format!(
            "/projects/{}/services/{}/summary",
            project.as_str(),
            project.as_str()
        );

        self.get(path).await
    }

    pub async fn create_project(&self, project: &ProjectName) -> Result<project::Response> {
        let path = format!("/projects/{}", project.as_str());

        self.post(path, Option::<String>::None)
            .await
            .context("failed to make create project request")?
            .to_json()
            .await
    }

    pub async fn clean_project(&self, project: &ProjectName) -> Result<Vec<String>> {
        let path = format!("/projects/{}/clean", project.as_str(),);

        self.post(path, Option::<String>::None)
            .await
            .context("failed to get clean output")?
            .to_json()
            .await
    }

    pub async fn get_project(&self, project: &ProjectName) -> Result<project::Response> {
        let path = format!("/projects/{}", project.as_str());

        self.get(path).await
    }

    pub async fn get_projects_list(&self) -> Result<Vec<project::Response>> {
        let path = "/projects".to_string();

        self.get(path).await
    }

    pub async fn get_projects_list_filtered(
        &self,
        filter: String,
    ) -> Result<Vec<project::Response>> {
        let path = format!("/projects/{filter}");

        self.get(path).await
    }

    pub async fn delete_project(&self, project: &ProjectName) -> Result<project::Response> {
        let path = format!("/projects/{}", project.as_str());

        self.delete(path).await
    }

    pub async fn get_secrets(&self, project: &ProjectName) -> Result<Vec<secret::Response>> {
        let path = format!(
            "/projects/{}/secrets/{}",
            project.as_str(),
            project.as_str()
        );

        self.get(path).await
    }

    pub async fn get_logs(
        &self,
        project: &ProjectName,
        deployment_id: &Uuid,
    ) -> Result<Vec<LogItem>> {
        let path = format!(
            "/projects/{}/deployments/{}/logs",
            project.as_str(),
            deployment_id
        );

        self.get(path).await
    }

    pub async fn get_logs_ws(
        &self,
        project: &ProjectName,
        deployment_id: &Uuid,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let path = format!(
            "/projects/{}/ws/deployments/{}/logs",
            project.as_str(),
            deployment_id
        );

        self.ws_get(path).await
    }

    pub async fn get_deployment_details(
        &self,
        project: &ProjectName,
        deployment_id: &Uuid,
    ) -> Result<deployment::Response> {
        let path = format!(
            "/projects/{}/deployments/{}",
            project.as_str(),
            deployment_id
        );

        self.get(path).await
    }

    async fn ws_get(&self, path: String) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let ws_scheme = self.api_url.clone().replace("http", "ws");
        let url = format!("{}{}", ws_scheme, path);
        let mut request = url.into_client_request()?;

        if let Some(ref api_key) = self.api_key {
            let auth_header = Authorization::bearer(api_key)?;
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

        let mut builder = Self::get_retry_client().get(url);

        builder = self.set_builder_auth(builder);

        builder
            .send()
            .await
            .context("failed to make get request")?
            .to_json()
            .await
    }

    async fn post<T: Into<Body>>(
        &self,
        path: String,
        body: Option<T>,
    ) -> Result<Response, reqwest_middleware::Error> {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = Self::get_retry_client().post(url);

        builder = self.set_builder_auth(builder);

        if let Some(body) = body {
            builder = builder.body(body);
            builder = builder.header("Transfer-Encoding", "chunked");
        }

        builder.send().await
    }

    async fn delete<M>(&self, path: String) -> Result<M>
    where
        M: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = Self::get_retry_client().delete(url);

        builder = self.set_builder_auth(builder);

        builder
            .send()
            .await
            .context("failed to make delete request")?
            .to_json()
            .await
    }

    fn set_builder_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        if let Some(ref api_key) = self.api_key {
            builder.bearer_auth(api_key)
        } else {
            builder
        }
    }

    fn get_retry_client() -> ClientWithMiddleware {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    }
}
