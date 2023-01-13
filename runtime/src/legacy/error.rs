use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Start error: {0}")]
    Start(#[from] shuttle_service::error::CustomError),
}
