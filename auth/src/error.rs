use std::error::Error as StdError;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use serde::{ser::SerializeMap, Serialize};
use shuttle_common::models::error::ApiError;
use stripe::StripeError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("User could not be found")]
    UserNotFound,
    #[error("API key is missing.")]
    KeyMissing,
    #[error("Unauthorized.")]
    Unauthorized,
    #[error("Forbidden.")]
    Forbidden,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
    #[error("Incomplete checkout session.")]
    Stripe(#[from] StripeError),
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
        let code = match self {
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::Unauthorized | Error::KeyMissing => StatusCode::UNAUTHORIZED,
            Error::Database(sqlx::Error::RowNotFound) | Error::UserNotFound => {
                StatusCode::NOT_FOUND
            }
            _ => {
                // We only want to emit error events for internal errors, not e.g. 404s.
                tracing::error!(error = %self, "control plane request error");

                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        ApiError {
            message: self.to_string(),
            status_code: code.as_u16(),
        }
        .into_response()
    }
}
