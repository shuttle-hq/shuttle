pub mod admin;
pub mod deployment;
pub mod error;
pub mod project;
pub mod resource;
pub mod service;
pub mod stats;
pub mod team;
pub mod user;

use anyhow::{Context, Result};
use async_trait::async_trait;
use http::StatusCode;
use serde::de::DeserializeOwned;
use tracing::trace;

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

        let str_repr =
            String::from_utf8(full.to_vec()).unwrap_or_else(|_| format!("[{} bytes]", full.len()));
        trace!(
            response = %str_repr,
            "parsing response to json"
        );

        if matches!(
            status_code,
            StatusCode::OK | StatusCode::SWITCHING_PROTOCOLS
        ) {
            serde_json::from_slice(&full).context("failed to parse a successful response")
        } else {
            trace!("parsing response to common error");
            let res: error::ApiError = match serde_json::from_slice(&full) {
                Ok(res) => res,
                _ => error::ApiError {
                    message: "Failed to parse response from the server.".to_string(),
                    status_code: status_code.as_u16(),
                },
            };

            Err(res.into())
        }
    }
}
