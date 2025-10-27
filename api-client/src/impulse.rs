use anyhow::Result;

use crate::{util::ParsedJson, ShuttleApiClient};

pub struct ImpulseClient {
    pub inner: ShuttleApiClient,
}

impl ImpulseClient {
    pub fn new(api_url: String, api_key: String, timeout: u64) -> Self {
        Self {
            inner: ShuttleApiClient::new(api_url, Some(api_key), None, Some(timeout)),
        }
    }

    pub async fn todo(&self) -> Result<ParsedJson<String>> {
        self.inner.get_json("/todo").await
    }
}
