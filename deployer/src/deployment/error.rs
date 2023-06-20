use crate::dal::DalError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error")]
    Dal(DalError),
    #[error("Missing IPv4 address in the persistence")]
    MissingIpv4Address,
    #[error("Error occurred when running a deployment: {0}")]
    Send(String),
    #[error("Error at service runtime: {0}")]
    Runtime(anyhow::Error),
    #[error("Error preparing the service runtime: {0}")]
    PrepareRun(String),
    #[error("Encountered IO error: {0}")]
    IoError(std::io::Error),
    #[error("Error during the service load phase: {0}")]
    Load(String),
    #[error("Error during the service run phase: {0}")]
    Start(String),
}

pub type Result<T> = std::result::Result<T, Error>;
