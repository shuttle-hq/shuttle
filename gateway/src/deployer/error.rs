use std::error::Error as StdError;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use serde::{ser::SerializeMap, Serialize};
use shuttle_common::models::error::ApiError;
use tracing::error;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Streaming error: {0}")]
    Streaming(#[from] axum::Error),
    #[error("Persistence failure: {0}")]
    Persistence(#[from] sqlx::Error),
    #[error("{0}")]
    ProxyFqdnMissing(String),
    #[error("{0}, try running `cargo shuttle deploy`")]
    NotFound(String),
    #[error("Invalid project name: {0}")]
    InvalidProjectName(#[from] shuttle_common::models::error::InvalidProjectName),
    #[error("Service query error: {0}")]
    GatewayServiceQuery(#[from] crate::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("type", &format!("{:?}", self))?;
        // use the error source if available, if not use display implementation
        map.serialize_entry("msg", &self.source().unwrap_or(self).to_string())?;
        map.end()
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!(error = &self as &dyn std::error::Error, "request error");

        let code = match self {
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        ApiError {
            message: self.to_string(),
            status_code: code.as_u16(),
        }
        .into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
