use thiserror::Error;
use tonic::Status;
use tracing::error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to create role")]
    CreateRole(String),

    #[error("failed to update role")]
    UpdateRole(String),

    #[error("failed to create DB")]
    CreateDB(String),

    #[error("unexpected error")]
    Unexpected(#[from] sqlx::Error),
}

unsafe impl Send for Error {}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        error!(error = &err as &dyn std::error::Error, "provision failed");
        Status::internal("failed to provision a database")
    }
}
