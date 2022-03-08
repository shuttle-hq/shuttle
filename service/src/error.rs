use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database resource error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rocket error: {0}")]
    Rocket(#[from] rocket::Error)
}
