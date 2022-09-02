use std::fmt::Write;
use std::fs::File;
use std::io::Read;

use anyhow::{anyhow, Context, Result};
use http_auth_basic::Credentials;
use reqwest::header::AUTHORIZATION;
use reqwest::{Body, Response, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use semver::Version;
use serde::Deserialize;
use shuttle_common::project::ProjectName;
use shuttle_common::{deployment, log, secret, service, ApiKey, ApiUrl};
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

    pub async fn auth(&self, username: String) -> Result<ApiKey> {
        let path = format!("/users/{}", username);

        let response = self
            .post(path, Option::<String>::None)
            .await
            .context("failed to get API key from Shuttle server")?;

        let response_status = response.status();
        let response_text = response.text().await?;

        if response_status == StatusCode::OK {
            return Ok(response_text);
        }

        error!(
            text = response_text,
            status = %response_status,
            "failed to authenticate with server"
        );
        Err(anyhow!("failed to authenticate with server",))
    }

    pub async fn deploy(
        &self,
        package_file: File,
        project: &ProjectName,
        no_test: bool,
    ) -> Result<deployment::Response> {
        let mut path = format!("/services/{}", project.as_str());

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
            .json()
            .await
            .context("could not parse server response")
    }

    pub async fn get_build_logs_ws(
        &self,
        deployment_id: &Uuid,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut ws_url = self.api_url.clone().replace("http", "ws");
        let _ = write!(ws_url, "/ws/deployments/{}/logs/build", deployment_id);

        let mut request = ws_url.into_client_request()?;

        if let Some(ref api_key) = self.api_key {
            let cred = Credentials::new(api_key, "");
            request
                .headers_mut()
                .append(AUTHORIZATION, cred.as_http_header().parse()?);
        }

        let (stream, _) = connect_async(request).await.with_context(|| {
            error!("failed to connect to build logs websocket");
            "could not connect to build logs websocket"
        })?;

        Ok(stream)
    }

    pub async fn delete_service(&self, project: &ProjectName) -> Result<service::Detailed> {
        let path = format!("/services/{}", project.as_str());

        self.delete(path).await
    }

    pub async fn get_service_details(&self, project: &ProjectName) -> Result<service::Detailed> {
        let path = format!("/services/{}", project.as_str());

        self.get(path).await
    }

    pub async fn get_service_summary(&self, project: &ProjectName) -> Result<service::Summary> {
        let path = format!("/services/{}/summary", project.as_str());

        self.get(path).await
    }

    pub async fn get_shuttle_service_version(&self) -> Result<Version> {
        let url = format!("{}/version", self.api_url);

        let response = Self::get_retry_client()
            .get(url)
            .send()
            .await
            .context("failed to get version from Shuttle server")?;

        let response_status = response.status();
        let response_text = response.text().await?;

        if response_status == StatusCode::OK {
            Ok(Version::parse(&response_text)?)
        } else {
            error!(
                text = response_text,
                status = %response_status,
                "failed to get shuttle version from server"
            );
            Err(anyhow!("failed to get shuttle version from server"))
        }
    }

    pub async fn get_secrets(&self, project: &ProjectName) -> Result<Vec<secret::Response>> {
        let path = format!("/secrets/{}", project.as_str());

        self.get(path).await
    }

    pub async fn get_runtime_logs(&self, deployment_id: &Uuid) -> Result<Vec<log::Item>> {
        let path = format!("/deployments/{}/logs/runtime", deployment_id);

        self.get(path).await
    }

    pub async fn get_runtime_logs_ws(
        &self,
        deployment_id: &Uuid,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut ws_url = self.api_url.clone().replace("http", "ws");
        let _ = write!(ws_url, "/ws/deployments/{}/logs/runtime", deployment_id);

        let mut request = ws_url.into_client_request()?;

        if let Some(ref api_key) = self.api_key {
            let cred = Credentials::new(api_key, "");
            request
                .headers_mut()
                .append(AUTHORIZATION, cred.as_http_header().parse()?);
        }

        let (stream, _) = connect_async(request).await.with_context(|| {
            error!("failed to connect to runtime logs websocket");
            "could not connect to runtime logs websocket"
        })?;

        Ok(stream)
    }

    pub async fn get_deployment_details(
        &self,
        deployment_id: &Uuid,
    ) -> Result<deployment::Response> {
        let path = format!("/deployments/{}", deployment_id);

        self.get(path).await
    }

    async fn get<M>(&self, path: String) -> Result<M>
    where
        M: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = Self::get_retry_client().get(url);

        if let Some(ref api_key) = self.api_key {
            builder = builder.basic_auth::<&str, &str>(api_key, None);
        }

        builder
            .send()
            .await
            .context("failed to make get request")?
            .json()
            .await
            .context("could not parse server json response for get request")
    }

    async fn post<T: Into<Body>>(
        &self,
        path: String,
        body: Option<T>,
    ) -> Result<Response, reqwest_middleware::Error> {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = Self::get_retry_client().post(url);

        if let Some(ref api_key) = self.api_key {
            builder = builder.basic_auth::<&str, &str>(api_key, None);
        }

        if let Some(body) = body {
            builder = builder.body(body);
        }

        builder.send().await
    }

    async fn delete<M>(&self, path: String) -> Result<M>
    where
        M: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.api_url, path);

        let mut builder = Self::get_retry_client().delete(url);

        if let Some(ref api_key) = self.api_key {
            builder = builder.basic_auth::<&str, &str>(api_key, None);
        }

        builder
            .send()
            .await
            .context("failed to make delete request")?
            .json()
            .await
            .context("could not parse server json response for delete request")
    }

    fn get_retry_client() -> ClientWithMiddleware {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    }
}
