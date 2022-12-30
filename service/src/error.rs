//! Types representing various errors that can occur in the process of building and deploying a service.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Panic occurred in shuttle_service::main`: {0}")]
    BuildPanic(String),
    #[error("Panic occurred in `Service::bind`: {0}")]
    BindPanic(String),
    #[error("Custom error: {0}")]
    Custom(#[from] CustomError),
}

pub type CustomError = anyhow::Error;
