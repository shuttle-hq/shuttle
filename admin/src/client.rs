use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
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
        self.post("/admin/revive").await
    }

    pub async fn acme_account_create(&self, email: &str) -> Result<serde_json::Value> {
        let path = format!("/admin/acme/{email}");
        self.post(&path).await
    }

    async fn post<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        trace!(self.api_key, "using api key");

        reqwest::Client::new()
            .post(format!("{}{}", self.api_url, path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to make post request")?
            .json()
            .await
            .context("failed to extract json body from post response")
    }
}
