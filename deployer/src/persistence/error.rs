use super::DeploymentState;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Resource recorder error: {0}")]
    ResourceRecorder(tonic::Status),
    #[error("Sending the state event failed: {0}")]
    CrossbeamChannelSendError(#[from] crossbeam_channel::SendError<DeploymentState>),
    #[error("Parsing error: {0}")]
    ParseError(String),
    #[error("Provisioner request failed: {0}")]
    Provisioner(tonic::Status),
}

pub type Result<T> = std::result::Result<T, Error>;
