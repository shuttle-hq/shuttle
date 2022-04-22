//! Types representing various errors that can occur in the process of building and deploying a service.

use thiserror::Error;
use std::convert::From;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Custom error: {0}")]
    Custom(#[from] CustomError)
}

pub type CustomError = anyhow::Error;

#[cfg(feature = "sqlx-postgres")]
impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Database(e.to_string())
    }
}
