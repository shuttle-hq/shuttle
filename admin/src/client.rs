use anyhow::Result;
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::models::project::{ComputeTier, ProjectResponse, ProjectUpdateRequest};

pub struct Client {
    pub inner: ShuttleApiClient,
}

impl Client {
    pub fn new(api_url: String, api_key: String, timeout: u64) -> Self {
        Self {
            inner: ShuttleApiClient::new(api_url, Some(api_key), None, Some(timeout)),
        }
    }

    pub async fn renew_old_certificates(&self) -> Result<serde_json::Value> {
        self.inner
            .put_json("/admin/certificates/renew", Option::<()>::None)
            .await
    }

    pub async fn update_project_compute_tier(
        &self,
        project_id: &str,
        compute_tier: ComputeTier,
    ) -> Result<ProjectResponse> {
        self.inner
            .put_json(
                format!("/projects/{project_id}"),
                Some(ProjectUpdateRequest {
                    compute_tier: Some(compute_tier),
                    ..Default::default()
                }),
            )
            .await
    }

    pub async fn gc_free_tier(&self, days: u32) -> Result<Vec<String>> {
        let path = format!("/admin/gc/free/{days}");
        self.inner.get_json(&path).await
    }

    pub async fn gc_shuttlings(&self, minutes: u32) -> Result<Vec<String>> {
        let path = format!("/admin/gc/shuttlings/{minutes}");
        self.inner.get_json(&path).await
    }
}
