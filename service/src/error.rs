//! Types representing various errors that can occur in the process of building and deploying a service.

use thiserror::Error;

/// An error that can occur in the process of building and deploying a service.
/// 
/// This is an enum encapsulating the various errors that can occur in the process of building and deploying a service.
/// 
#[derive(Debug, Error)]
pub enum Error {

    /// An Input/Output error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// An Error related to the database.
    #[error("Database error: {0}")]
    Database(String),
    /// An error related to the build process.
    #[error("Panic occurred in shuttle_service::main`: {0}")]
    BuildPanic(String),
    /// An error related to the bind process.
    #[error("Panic occurred in `Service::bind`: {0}")]
    BindPanic(String),
    /// An error related to parsing the Secrets.toml file.
    #[error("Failed to interpolate string. Is your Secrets.toml correct?")]
    StringInterpolation(#[from] strfmt::FmtError),
    /// A custom error case not covered by the other variants.
    #[error("Custom error: {0}")]
    Custom(#[from] CustomError),
}

pub type CustomError = anyhow::Error;
