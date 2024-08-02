use anyhow::{Context, Result};
use async_trait::async_trait;
use http::StatusCode;
use serde::de::DeserializeOwned;
use shuttle_common::models::error::ApiError;

/// A to_json wrapper for handling our error states
#[async_trait]
pub trait ToJson {
    async fn to_json<T: DeserializeOwned>(self) -> Result<T>;
}

#[async_trait]
impl ToJson for reqwest::Response {
    async fn to_json<T: DeserializeOwned>(self) -> Result<T> {
        let status_code = self.status();
        let full = self.bytes().await?;

        #[cfg(feature = "tracing")]
        tracing::trace!(
            response = %String::from_utf8(full.to_vec()).unwrap_or_else(|_| format!("[{} bytes]", full.len())),
            "parsing response to json"
        );

        if matches!(
            status_code,
            StatusCode::OK | StatusCode::SWITCHING_PROTOCOLS
        ) {
            serde_json::from_slice(&full).context("failed to parse a successful response")
        } else {
            #[cfg(feature = "tracing")]
            tracing::trace!("parsing response to common error");

            let res: ApiError = match serde_json::from_slice(&full) {
                Ok(res) => res,
                _ => ApiError {
                    message: "Failed to parse response from the server.".to_string(),
                    status_code: status_code.as_u16(),
                },
            };

            Err(res.into())
        }
    }
}
