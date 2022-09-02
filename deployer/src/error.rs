use std::error::Error as StdError;
use std::io;

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use serde::{ser::SerializeMap, Serialize};
use shuttle_service::loader::LoaderError;

use cargo::util::errors::CargoTestError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Streaming error: {0}")]
    Streaming(#[source] axum::Error),
    #[error("Internal I/O error: {0}")]
    InputOutput(#[from] io::Error),
    #[error("Build error: {0}")]
    Build(#[source] Box<dyn StdError + Send>),
    #[error("Prepare to load error: {0}")]
    PrepareLoad(String),
    #[error("Load error: {0}")]
    Load(#[from] LoaderError),
    #[error("Run error: {0}")]
    Run(#[from] shuttle_service::Error),
    #[error("Pre-deployment test failure: {0}")]
    PreDeployTestFailure(#[from] CargoTestError),
    #[error("Failed to parse secrets: {0}")]
    SecretsParse(#[from] toml::de::Error),
    #[error("Failed to set secrets: {0}")]
    SecretsSet(#[source] Box<dyn StdError + Send>),
    #[error("Failed to parse secrets: {0}")]
    Persistence(#[from] crate::persistence::PersistenceError),
    #[error("Not found")]
    NotFound,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("type", &format!("{:?}", self))?;
        map.serialize_entry("msg", &self.source().unwrap().to_string())?;
        map.end()
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (
            status_code,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            Json(self),
        )
            .into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
