use anyhow::{bail, Context, Result};
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};
use shuttle_common::models::{admin::ProjectResponse, stats, ToJson};
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

    pub async fn destroy(&self) -> Result<String> {
        self.post("/admin/destroy", Option::<String>::None).await
    }

    pub async fn idle_cch(&self) -> Result<()> {
        reqwest::Client::new()
            .post(format!("{}/admin/idle-cch", self.api_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to send idle request")?;

        Ok(())
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
        project_name: &str,
        credentials: &serde_json::Value,
    ) -> Result<String> {
        let path = format!("/admin/acme/request/{project_name}/{fqdn}");
        self.post(&path, Some(credentials)).await
    }

    pub async fn acme_renew_custom_domain_certificate(
        &self,
        fqdn: &str,
        project_name: &str,
        credentials: &serde_json::Value,
    ) -> Result<String> {
        let path = format!("/admin/acme/renew/{project_name}/{fqdn}");
        self.post(&path, Some(credentials)).await
    }

    pub async fn acme_renew_gateway_certificate(
        &self,
        credentials: &serde_json::Value,
    ) -> Result<String> {
        let path = "/admin/acme/gateway/renew".to_string();
        self.post(&path, Some(credentials)).await
    }

    pub async fn get_projects(&self) -> Result<Vec<ProjectResponse>> {
        self.get("/admin/projects").await
    }

    pub async fn change_project_owner(&self, project_name: &str, new_user_id: &str) -> Result<()> {
        self.get_raw(&format!(
            "/admin/projects/change-owner/{project_name}/{new_user_id}"
        ))
        .await?;

        Ok(())
    }

    pub async fn get_load(&self) -> Result<stats::LoadResponse> {
        self.get("/admin/stats/load").await
    }

    pub async fn clear_load(&self) -> Result<stats::LoadResponse> {
        self.delete("/admin/stats/load", Option::<String>::None)
            .await
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

    async fn delete<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<T>,
    ) -> Result<R> {
        trace!(self.api_key, "using api key");

        let mut builder = reqwest::Client::new()
            .delete(format!("{}{}", self.api_url, path))
            .bearer_auth(&self.api_key);

        if let Some(body) = body {
            builder = builder.json(&body);
        }

        builder
            .send()
            .await
            .context("failed to make delete request")?
            .to_json()
            .await
            .context("failed to extract json body from delete response")
    }

    async fn get_raw(&self, path: &str) -> Result<Bytes> {
        let res = reqwest::Client::new()
            .get(format!("{}{}", self.api_url, path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("making request")?;
        if !res.status().is_success() {
            bail!("API call returned non-2xx: {:?}", res);
        }
        res.bytes().await.context("getting response body")
    }

    async fn get<R: DeserializeOwned>(&self, path: &str) -> Result<R> {
        serde_json::from_slice(&self.get_raw(path).await?).context("deserializing body")
    }
}
