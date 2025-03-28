use anyhow::Result;
use serde_json::{json, Value};
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::models::{
    project::{ProjectResponse, ProjectUpdateRequest},
    user::UserResponse,
};

pub struct Client {
    pub inner: ShuttleApiClient,
}

impl Client {
    pub fn new(api_url: String, api_key: String, timeout: u64) -> Self {
        Self {
            inner: ShuttleApiClient::new(api_url, Some(api_key), None, Some(timeout)),
        }
    }

    pub async fn get_old_certificates(&self) -> Result<Vec<(String, String)>> {
        self.inner.get_json("/admin/certificates").await
    }

    pub async fn renew_certificate(&self, cert_id: &str) -> Result<String> {
        self.inner
            .put_json(
                format!("/admin/certificates/renew/{cert_id}"),
                Option::<()>::None,
            )
            .await
    }

    pub async fn update_project_config(
        &self,
        project_id: &str,
        config: serde_json::Value,
    ) -> Result<ProjectResponse> {
        self.inner
            .put_json(
                format!("/projects/{project_id}"),
                Some(ProjectUpdateRequest {
                    config: Some(config),
                    ..Default::default()
                }),
            )
            .await
    }

    pub async fn get_project_config(&self, project_id: &str) -> Result<Value> {
        self.inner
            .get_json(format!("/admin/projects/{project_id}"))
            .await
    }

    pub async fn upgrade_project_to_lb(&self, project_id: &str) -> Result<Value> {
        self.inner
            .put_json(format!("/admin/projects/{project_id}"), Option::<()>::None)
            .await
    }

    pub async fn update_lb_project_replicas(
        &self,
        project_id: &str,
        replicas: u8,
    ) -> Result<Value> {
        self.inner
            .put_json(
                format!("/admin/projects/{project_id}/{replicas}"),
                Option::<()>::None,
            )
            .await
    }

    pub async fn update_project_owner(
        &self,
        project_id: &str,
        user_id: String,
    ) -> Result<ProjectResponse> {
        self.inner
            .put_json(
                format!("/projects/{project_id}"),
                Some(ProjectUpdateRequest {
                    user_id: Some(user_id),
                    ..Default::default()
                }),
            )
            .await
    }

    pub async fn feature_flag(&self, entity: &str, flag: &str, set: bool) -> Result<()> {
        let resp = if set {
            self.inner
                .put(
                    format!("/admin/feature-flag/{entity}/{flag}"),
                    Option::<()>::None,
                )
                .await?
        } else {
            self.inner
                .delete(
                    format!("/admin/feature-flag/{entity}/{flag}"),
                    Option::<()>::None,
                )
                .await?
        };

        if !resp.status().is_success() {
            dbg!(resp);
            panic!("request failed");
        }

        Ok(())
    }

    pub async fn gc_free_tier(&self, days: u32) -> Result<Vec<String>> {
        let path = format!("/admin/gc/free/{days}");
        self.inner.get_json(&path).await
    }

    pub async fn gc_shuttlings(&self, minutes: u32) -> Result<Vec<String>> {
        let path = format!("/admin/gc/shuttlings/{minutes}");
        self.inner.get_json(&path).await
    }

    pub async fn get_user(&self, user_id: &str) -> Result<UserResponse> {
        self.inner.get_json(format!("/admin/users/{user_id}")).await
    }

    pub async fn get_user_everything(&self, query: &str) -> Result<Value> {
        self.inner
            .get_json_with_body("/admin/users/everything", json!(query))
            .await
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<String> {
        self.inner
            .delete_json(format!("/admin/users/{user_id}"))
            .await
    }

    pub async fn set_user_tier(&self, user_id: &str, tier: &str) -> Result<()> {
        self.inner
            .put(
                format!("/admin/users/{user_id}/tier/{tier}"),
                Option::<()>::None,
            )
            .await?;

        Ok(())
    }
}
