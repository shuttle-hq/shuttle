#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Unexpected error: {0}")]
    Unexpected(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
