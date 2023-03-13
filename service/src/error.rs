//! Types representing various errors that can occur in the process of building and deploying a service.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Panic occurred in shuttle_service::main`: {0}")]
    BuildPanic(String),
    #[error("Panic occurred in `Service::bind`: {0}")]
    BindPanic(String),
    #[error("Failed to interpolate string. Is your Secrets.toml correct?")]
    StringInterpolation(#[from] strfmt::FmtError),
    #[error("Custom error: {0}")]
    Custom(#[from] CustomError),
}

pub type CustomError = anyhow::Error;
