use super::persistence::dal::DalError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error")]
    Dal(DalError),
    #[error("Missing IPv4 address in the persistence")]
    MissingIpv4Address,
    #[error("Error occurred when running a deployment: {0}")]
    Send(String),
}
