use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database resource error: {0}")]
    Database(#[from] sqlx::Error),
}
