use ulid::DecodeError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Ulid error: {0}")]
    Ulid(#[from] DecodeError),
}