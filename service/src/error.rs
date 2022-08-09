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
    #[error("Custom error: {0}")]
    Custom(#[from] CustomError),
}

pub type CustomError = anyhow::Error;

// This is implemented manually as defining `Error::Database(#[from] sqlx::Error)` resulted in a
// segfault even with a feature guard.
#[cfg(feature = "sqlx-postgres")]
impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Database(e.to_string())
    }
}
// TODO: does this have to be implemented manually?
#[cfg(feature = "mongodb-integration")]
impl From<mongodb::error::Error> for Error {
    fn from(e: mongodb::error::Error) -> Self {
        Error::Database(e.to_string())
    }
}
