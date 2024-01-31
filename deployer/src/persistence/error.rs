use super::DeploymentState;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Resource recorder error: {0}")]
    ResourceRecorder(tonic::Status),
    #[error("Resource recorder sync error")]
    ResourceRecorderSync,
    #[error("Sending the state event failed: {0}")]
    ChannelSendError(#[from] tokio::sync::mpsc::error::SendError<DeploymentState>),
    #[error("Sending the state event failed: channel closed")]
    ChannelSendThreadError,
    #[error("Parsing error: {0}")]
    ParseError(String),
    #[error("Provisioner request failed: {0}")]
    Provisioner(tonic::Status),
    #[error("SQLite persistence error: {0}")]
    LocalPersistence(#[from] shuttle_common::persistence::Error),
    #[error("Remote persistence error: {0}")]
    RemotePersistence(#[from] shuttle_common::backends::client::Error),
    #[error("Persistence mode error: {0}")]
    Mode(String),
}

pub type Result<T> = std::result::Result<T, Error>;
