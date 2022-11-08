use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use tracing::trace;

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
        credentials: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = format!("/admin/acme/request/{fqdn}");
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
            .json()
            .await
            .context("failed to extract json body from post response")
    }
}
