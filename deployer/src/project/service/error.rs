use tracing::error;

use bollard::errors::Error as DockerError;

use super::state::m_errored::ServiceErrored;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to prepare the shuttle runtime: {0}")]
    RuntimePrepare(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Docker error: {0}")]
    Docker(DockerError),
    #[error("Invalid project name")]
    InvalidProjectName,
    #[error("State internal error: {0}")]
    Internal(String),
    #[error("Service error: {0}")]
    Service(ServiceErrored),
    #[error("Ulid decode error: {0}")]
    Decode(ulid::DecodeError),
}

impl From<ServiceErrored> for Error {
    fn from(err: ServiceErrored) -> Self {
        Self::Service(err)
    }
}
