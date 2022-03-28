//! Types representing various errors that can occur in the process of building and deploying a service.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rocket error: {0}")]
    Rocket(#[from] Box<rocket::Error>),
    #[error("Custom error: {0}")]
    Custom(#[from] CustomError)
}

impl From<rocket::Error> for Error {
    fn from(error: rocket::Error) -> Self {
        Self::from(Box::new(error))
    }
}

pub type CustomError = anyhow::Error;
