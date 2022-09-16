use std::error::Error as StdError;

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use serde::{ser::SerializeMap, Serialize};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Persistence failure: {0}")]
    Persistence(#[from] crate::persistence::PersistenceError),
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            Json(json!({ "message": self })),
        )
            .into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
