use std::{error::Error as StdError, fmt::Formatter};

use crate::dal::DalError;

/// A wrapper to capture any error possible with this service
#[derive(Debug)]
pub enum Error {
    TaskInternal,
    ServiceUnavailable,
    Dal(DalError),
    Service(super::service::error::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::TaskInternal => write!(f, "task internal error"),
            Self::ServiceUnavailable => write!(f, "user service is unavailable"),
            Self::Dal(_) => write!(f, "persistence error triggered by service state machine"),
            Self::Service(err) => write!(f, "{}", err),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl StdError for Error {}
