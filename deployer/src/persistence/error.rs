#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Resource recorder client error: {0}")]
    ResourceRecorder(#[from] tonic::Status),
}

pub type Result<T> = std::result::Result<T, Error>;
