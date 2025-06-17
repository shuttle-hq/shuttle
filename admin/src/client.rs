use anyhow::Result;
use serde_json::{json, Value};
use shuttle_api_client::{
    util::{ParsedJson, ToBodyContent},
    ShuttleApiClient,
};
use shuttle_common::models::{
    project::{ProjectResponse, ProjectUpdateRequest},
    team::AddTeamMemberRequest,
    user::{AccountTier, UpdateAccountTierRequest, UserResponse},
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

    pub async fn get_old_certificates(
        &self,
    ) -> Result<ParsedJson<Vec<(String, String, Option<String>)>>> {
        self.inner.get_json("/admin/certificates").await
    }

    pub async fn renew_certificate(&self, cert_id: &str) -> Result<ParsedJson<String>> {
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
    ) -> Result<ParsedJson<ProjectResponse>> {
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

    pub async fn get_project_config(&self, project_id: &str) -> Result<ParsedJson<Value>> {
        self.inner
            .get_json(format!("/admin/projects/{project_id}/config"))
            .await
    }

    pub async fn upgrade_project_to_lb(&self, project_id: &str) -> Result<ParsedJson<Value>> {
        self.inner
            .put_json(
                format!("/admin/projects/{project_id}/config"),
                Option::<()>::None,
            )
            .await
    }

    pub async fn update_project_scale(
        &self,
        project_id: &str,
        update_config: &Value,
    ) -> Result<ParsedJson<Value>> {
        self.inner
            .put_json(
                format!("/admin/projects/{project_id}/scale"),
                Some(update_config),
            )
            .await
    }

    pub async fn update_project_owner(
        &self,
        project_id: &str,
        user_id: String,
    ) -> Result<ParsedJson<ProjectResponse>> {
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

    pub async fn add_team_member(
        &self,
        team_user_id: &str,
        user_id: String,
    ) -> Result<ParsedJson<String>> {
        self.inner
            .post_json(
                format!("/teams/{team_user_id}/members"),
                Some(AddTeamMemberRequest {
                    user_id: Some(user_id),
                    email: None,
                    role: None,
                }),
            )
            .await
    }

    pub async fn feature_flag(&self, entity: &str, flag: &str, set: bool) -> Result<()> {
        if set {
            self.inner
                .put(
                    format!("/admin/feature-flag/{entity}/{flag}"),
                    Option::<()>::None,
                )
                .await?
                .to_empty()
                .await
        } else {
            self.inner
                .delete(
                    format!("/admin/feature-flag/{entity}/{flag}"),
                    Option::<()>::None,
                )
                .await?
                .to_empty()
                .await
        }
    }

    pub async fn gc_free_tier(&self, days: u32) -> Result<ParsedJson<Vec<String>>> {
        let path = format!("/admin/gc/free/{days}");
        self.inner.get_json(&path).await
    }

    pub async fn gc_shuttlings(&self, minutes: u32) -> Result<ParsedJson<Vec<String>>> {
        let path = format!("/admin/gc/shuttlings/{minutes}");
        self.inner.get_json(&path).await
    }

    pub async fn stop_gc_inactive_project(&self, project_id: &str) -> Result<ParsedJson<String>> {
        let path = format!("/admin/gc/stop-inactive-project/{project_id}");
        self.inner.put_json(&path, Option::<()>::None).await
    }

    pub async fn get_user(&self, user_id: &str) -> Result<ParsedJson<UserResponse>> {
        self.inner.get_json(format!("/admin/users/{user_id}")).await
    }

    pub async fn get_user_everything(&self, query: &str) -> Result<ParsedJson<Value>> {
        self.inner
            .get_json_with_body("/admin/users/everything", json!(query))
            .await
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<ParsedJson<String>> {
        self.inner
            .delete_json(format!("/admin/users/{user_id}"))
            .await
    }

    pub async fn set_user_tier(&self, user_id: &str, account_tier: AccountTier) -> Result<()> {
        self.inner
            .put(
                format!("/admin/users/{user_id}/account_tier"),
                Some(UpdateAccountTierRequest { account_tier }),
            )
            .await?
            .to_empty()
            .await
    }

    pub async fn get_expired_protrials(&self) -> Result<ParsedJson<Vec<String>>> {
        self.inner.get_json("/admin/users/protrial-downgrade").await
    }

    pub async fn downgrade_protrial(&self, user_id: &str) -> Result<ParsedJson<String>> {
        self.inner
            .put_json(
                format!("/admin/users/protrial-downgrade/{user_id}"),
                Option::<()>::None,
            )
            .await
    }

    pub async fn get_projects_for_retention_policy(&self) -> Result<ParsedJson<Vec<String>>> {
        self.inner.get_json("/admin/log-retention").await
    }
    pub async fn fix_retention_policy(&self, project_id: &str) -> Result<ParsedJson<String>> {
        self.inner
            .put_json(
                format!("/admin/log-retention/{project_id}"),
                Option::<()>::None,
            )
            .await
    }
}
