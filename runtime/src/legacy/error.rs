use shuttle_service::loader::LoaderError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Load error: {0}")]
    Load(#[from] LoaderError),
    #[error("Run error: {0}")]
    Run(#[from] shuttle_service::Error),
    #[error("Start error: {0}")]
    Start(#[from] shuttle_service::error::CustomError),
}

pub type Result<T> = std::result::Result<T, Error>;
