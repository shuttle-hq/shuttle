pub mod deployment;
pub mod error;
pub mod project;
pub mod resource;
pub mod secret;
pub mod service;
pub mod stats;
pub mod user;

use anyhow::{Context, Result};
use async_trait::async_trait;
use http::StatusCode;
use serde::de::DeserializeOwned;
use thiserror::Error;
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

        trace!(
            response = std::str::from_utf8(&full).unwrap_or_default(),
            "parsing response to json"
        );

        if matches!(
            status_code,
            StatusCode::OK | StatusCode::SWITCHING_PROTOCOLS
        ) {
            serde_json::from_slice(&full).context("failed to parse a successfull response")
        } else {
            trace!("parsing response to common error");
            let res: error::ApiError = match serde_json::from_slice(&full) {
                Ok(res) => res,
                _ => {
                    trace!("getting error from status code");
                    status_code.into()
                }
            };

            Err(res.into())
        }
    }
}

/// Errors that can occur when changing types. Especially from prost
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("failed to parse timestamp: {0}")]
    Timestamp(#[from] prost_types::TimestampError),
    #[error("failed to parse serde: {0}")]
    Serde(#[from] serde_json::Error),
}
