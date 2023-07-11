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
    #[error("Parsing error: {0}")]
    Parse(String),
    #[error("Missing container inspect info: {0}")]
    MissingContainerInspectInfo(String),
}

impl From<ServiceErrored> for Error {
    fn from(err: ServiceErrored) -> Self {
        Self::Service(err)
    }
}
