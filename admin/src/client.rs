use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::{Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use shuttle_common::{models::error, project::ProjectName};
use tracing::trace;

#[async_trait]
trait ToJson {
    async fn to_json<T: DeserializeOwned>(self) -> Result<T>;
}

#[async_trait]
impl ToJson for Response {
    async fn to_json<T: DeserializeOwned>(self) -> Result<T> {
        let status_code = self.status();
        let full = self.bytes().await?;

        trace!(
            response = std::str::from_utf8(&full).unwrap_or_default(),
            "parsing response to json"
        );

        if matches!(
            status_code,
            StatusCode::OK | StatusCode::SWITCHING_PROTOCOLS
        ) {
            serde_json::from_slice(&full).context("failed to parse a successfull response")
        } else {
            trace!("parsing response to common error");
            let res: error::ApiError = match serde_json::from_slice(&full) {
                Ok(res) => res,
                _ => {
                    trace!("getting error from status code");
                    panic!("fire");
                }
            };

            Err(res.into())
        }
    }
}

pub struct Client {
    api_url: String,
    api_key: String,
}

impl Client {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self { api_url, api_key }
    }

    pub async fn revive(&self) -> Result<String> {
        self.post("/admin/revive", Option::<String>::None).await
    }

    pub async fn acme_account_create(&self, email: &str) -> Result<serde_json::Value> {
        let path = format!("/admin/acme/{email}");
        self.post(&path, Option::<String>::None).await
    }

    pub async fn acme_request_certificate(
        &self,
        fqdn: &str,
        project_name: &ProjectName,
        credentials: &serde_json::Value,
    ) -> Result<String> {
        let path = format!("/admin/acme/request/{project_name}/{fqdn}");
        self.post(&path, Some(credentials)).await
    }

    async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<T>,
    ) -> Result<R> {
        trace!(self.api_key, "using api key");

        let mut builder = reqwest::Client::new()
            .post(format!("{}{}", self.api_url, path))
            .bearer_auth(&self.api_key);

        if let Some(body) = body {
            builder = builder.json(&body);
        }

        builder
            .send()
            .await
            .context("failed to make post request")?
            .to_json()
            .await
            .context("failed to extract json body from post response")
    }
}
