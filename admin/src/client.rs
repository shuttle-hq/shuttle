use anyhow::{Context, Result};

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

    async fn post(&self, path: &str) -> Result<String> {
        reqwest::Client::new()
            .post(format!("{}{}", self.api_url, path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to make post request")?
            .text()
            .await
            .context("failed to post text body from response")
    }
}
