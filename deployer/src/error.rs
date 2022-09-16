use std::error::Error as StdError;
use std::io;

use shuttle_service::loader::LoaderError;

use cargo::util::errors::CargoTestError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Streaming error: {0}")]
    Streaming(#[source] axum::Error),
    #[error("Internal I/O error: {0}")]
    InputOutput(#[from] io::Error),
    #[error("Build error: {0}")]
    Build(#[source] Box<dyn StdError + Send>),
    #[error("Prepare to load error: {0}")]
    PrepareLoad(String),
    #[error("Load error: {0}")]
    Load(#[from] LoaderError),
    #[error("Run error: {0}")]
    Run(#[from] shuttle_service::Error),
    #[error("Pre-deployment test failure: {0}")]
    PreDeployTestFailure(#[from] CargoTestError),
    #[error("Failed to parse secrets: {0}")]
    SecretsParse(#[from] toml::de::Error),
    #[error("Failed to set secrets: {0}")]
    SecretsSet(#[source] Box<dyn StdError + Send>),
}

pub type Result<T> = std::result::Result<T, Error>;
