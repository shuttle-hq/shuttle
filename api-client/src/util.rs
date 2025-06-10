use std::fmt::Debug;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use http::StatusCode;
use serde::de::DeserializeOwned;
use shuttle_common::models::error::ApiError;

/// Helpers for consuming and parsing response bodies and handling parsing of an ApiError if the response is 4xx/5xx
#[async_trait]
pub trait ToBodyContent {
    async fn to_json<T: DeserializeOwned>(self) -> Result<ParsedJson<T>>;
    async fn to_text(self) -> Result<String>;
    async fn to_bytes(self) -> Result<Bytes>;
    async fn to_empty(self) -> Result<()>;
}

fn into_api_error(body: &str, status_code: StatusCode) -> ApiError {
    #[cfg(feature = "tracing")]
    tracing::trace!("Parsing response as API error");

    let res: ApiError = match serde_json::from_str(body) {
        Ok(res) => res,
        _ => ApiError::new(
            format!("Failed to parse error response from the server:\n{}", body),
            status_code,
        ),
    };

    res
}

/// Tries to convert bytes to string. If not possible, returns a string symbolizing the bytes and the length
fn bytes_to_string_with_fallback(bytes: Bytes) -> String {
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| format!("[{} bytes]", bytes.len()))
}

pub struct ParsedJson<T> {
    inner: T,
    pub raw_json: String,
}

impl<T> ParsedJson<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }
    pub fn into_parts(self) -> (T, String) {
        (self.inner, self.raw_json)
    }
}

impl<T> AsRef<T> for ParsedJson<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T: Debug> Debug for ParsedJson<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
impl<T: std::fmt::Display> std::fmt::Display for ParsedJson<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

#[async_trait]
impl ToBodyContent for reqwest::Response {
    async fn to_json<T: DeserializeOwned>(self) -> Result<ParsedJson<T>> {
        let status_code = self.status();
        let bytes = self.bytes().await?;
        let string = bytes_to_string_with_fallback(bytes);

        #[cfg(feature = "tracing")]
        tracing::trace!(response = %string, "Parsing response as JSON");

        if status_code.is_client_error() || status_code.is_server_error() {
            return Err(into_api_error(&string, status_code).into());
        }

        let t = serde_json::from_str(&string).context("failed to parse a successful response")?;

        Ok(ParsedJson {
            inner: t,
            raw_json: string,
        })
    }

    async fn to_text(self) -> Result<String> {
        let status_code = self.status();
        let bytes = self.bytes().await?;
        let string = bytes_to_string_with_fallback(bytes);

        #[cfg(feature = "tracing")]
        tracing::trace!(response = %string, "Parsing response as text");

        if status_code.is_client_error() || status_code.is_server_error() {
            return Err(into_api_error(&string, status_code).into());
        }

        Ok(string)
    }

    async fn to_bytes(self) -> Result<Bytes> {
        let status_code = self.status();
        let bytes = self.bytes().await?;

        #[cfg(feature = "tracing")]
        tracing::trace!(response_length = bytes.len(), "Got response bytes");

        if status_code.is_client_error() || status_code.is_server_error() {
            let string = bytes_to_string_with_fallback(bytes);
            return Err(into_api_error(&string, status_code).into());
        }

        Ok(bytes)
    }

    async fn to_empty(self) -> Result<()> {
        let status_code = self.status();

        if status_code.is_client_error() || status_code.is_server_error() {
            let bytes = self.bytes().await?;
            let string = bytes_to_string_with_fallback(bytes);
            return Err(into_api_error(&string, status_code).into());
        }

        Ok(())
    }
}
