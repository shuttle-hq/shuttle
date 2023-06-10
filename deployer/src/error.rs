use crate::dal::DalError;

/// A wrapper to capture any error possible with this service
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to interact with database: {0}")]
    Dal(#[from] DalError),
}

pub type Result<T> = std::result::Result<T, Error>;
