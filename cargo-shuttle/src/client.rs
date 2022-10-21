use std::fmt::Write;
use std::fs::File;
use std::io::Read;

use anyhow::{Context, Result};
use async_trait::async_trait;
use headers::{Authorization, HeaderMapExt};
use reqwest::{Body, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use shuttle_common::models::{deployment, error, project, secret, service, user};
use shuttle_common::project::ProjectName;
use shuttle_common::{ApiKey, ApiUrl, LogItem};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{error, trace};
use uuid::Uuid;

pub struct Client {
    api_url: ApiUrl,
    api_key: Option<ApiKey>,
}

#[async_trait]
trait ToJson {
    async fn to_json<T: DeserializeOwned>(self) -> Result<T>;
}

#[async_trait]
impl ToJson for Response {
    async fn to_json<T: DeserializeOwned>(self) -> Result<T> {
        let full = self.bytes().await?;

        trace!(
            response = std::str::from_utf8(&full).unwrap_or_default(),
            "parsing response to json"
        );
        // try to deserialize into calling function response model
        match serde_json::from_slice(&full) {
            Ok(res) => Ok(res),
            Err(_) => {
                trace!("parsing response to common error");
                // if that doesn't work, try to deserialize into common error type
                let res: error::ApiError =
                    serde_json::from_slice(&full).context("failed to parse response to JSON")?;

                Err(res.into())
            }
        }
    }
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

    pub async fn auth(&self, username: String) -> Result<user::Response> {
        let path = format!("/users/{}", username);

        self.post(path, Option::<String>::None)
            .await
            .context("failed to get API key from Shuttle server")?
            .to_json()
            .await
    }

    pub async fn deploy(
        &self,
        package_file: File,
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

        let mut package_file = package_file;
        let mut package_content = Vec::new();
        package_file
            .read_to_end(&mut package_content)
            .context("failed to convert package content to buf")?;

        self.post(path, Some(package_content))
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

    pub async fn get_project(&self, project: &ProjectName) -> Result<project::Response> {
        let path = format!("/projects/{}", project.as_str());

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
