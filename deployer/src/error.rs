use std::error::Error as StdError;
use std::io;
use thiserror::Error;

use cargo::util::errors::CliError;

use crate::deployment::gateway_client;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Internal I/O error: {0}")]
    InputOutput(#[from] io::Error),
    #[error("Build error: {0}")]
    Build(#[source] Box<dyn StdError + Send>),
    #[error("Load error: {0}")]
    Load(String),
    #[error("Prepare to run error: {0}")]
    PrepareRun(String),
    #[error("Run error: {0}")]
    Run(#[from] shuttle_service::Error),
    #[error("Pre-deployment test failure: {0}")]
    PreDeployTestFailure(#[from] TestError),
    #[error("Failed to parse secrets: {0}")]
    SecretsParse(#[from] toml::de::Error),
    #[error("Failed to set secrets: {0}")]
    SecretsSet(#[source] Box<dyn StdError + Send>),
    #[error("Failed to get secrets: {0}")]
    SecretsGet(#[source] Box<dyn StdError + Send>),
    #[error("Failed to cleanup old deployments: {0}")]
    OldCleanup(#[source] Box<dyn StdError + Send>),
    #[error("Gateway client error: {0}")]
    GatewayClient(#[from] gateway_client::Error),
    #[error("Failed to get runtime: {0}")]
    Runtime(#[source] anyhow::Error),
    #[error("Failed to call start on runtime: {0}")]
    Start(String),
}

#[derive(Error, Debug)]
pub enum TestError {
    #[error("Tests failed")]
    Failed(CliError),
    #[error("Failed to setup tests run: {0}")]
    Setup(#[from] anyhow::Error),
    #[error("Failed to run tests: {0}")]
    Run(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, Error>;
