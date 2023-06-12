use tracing::error;

use super::state::errored::ServiceErrored;
use bollard::errors::Error as DockerError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to prepare the shuttle runtime: {0}")]
    RuntimePrepare(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Docker error: {0}")]
    Docker(DockerError),
}
