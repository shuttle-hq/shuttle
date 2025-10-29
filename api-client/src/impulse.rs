use anyhow::Result;

use crate::{util::ToBodyContent, ShuttleApiClient};

pub struct ImpulseClient {
    pub inner: ShuttleApiClient,
}

impl ImpulseClient {
    pub fn new(api_url: String, api_key: String, timeout: u64) -> Self {
        Self {
            inner: ShuttleApiClient::new(api_url, Some(api_key), None, Some(timeout)),
        }
    }

    pub async fn get_agents_md(&self) -> Result<String> {
        self.inner
            .get("/todo", Option::<()>::None)
            .await?
            .to_text()
            .await
    }
}
