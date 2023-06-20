use crate::dal::DalError;

/// A wrapper to capture any error possible with this service
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to interact with database")]
    Dal(#[from] DalError),
    #[error("Ulid decode error")]
    UlidDecode(ulid::DecodeError),
    #[error("Service already exists")]
    ServiceAlreadyExists,
    #[error("Service is missing IPv4 address")]
    MissingIpv4Address,
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;
