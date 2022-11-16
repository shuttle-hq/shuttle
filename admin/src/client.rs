use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use shuttle_common::{models::ToJson, project::ProjectName};
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

    pub async fn acme_account_create(
        &self,
        email: &str,
        acme_server: Option<String>,
    ) -> Result<serde_json::Value> {
        let path = format!("/admin/acme/{email}");
        self.post(&path, Some(acme_server)).await
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

    pub async fn list_invalid_project_names(&self) -> Result<Vec<(String, Vec<String>)>> {
        self.get("/admin/invalid-names").await
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

    async fn get<R: DeserializeOwned>(&self, path: &str) -> Result<R> {
        reqwest::Client::new()
            .get(format!("{}{}", self.api_url, path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to make post request")?
            .to_json()
            .await
            .context("failed to post text body from response")
    }
}
