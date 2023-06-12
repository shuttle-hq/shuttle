/// A wrapper to capture any error possible with this service
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Old cleanup : {0}")]
    OldCleanup(Box<dyn std::error::Error + Send>),
    #[error("Run error: {0}")]
    Run(anyhow::Error),
    #[error("Runtime error: {0}")]
    Runtime(anyhow::Error),
    #[error("Prepare run: {0}")]
    PrepareRun(String),
    #[error("IO error: {0}")]
    IoError(std::io::Error),
    #[error("Secrets get: {0}")]
    SecretsGet(Box<dyn std::error::Error + Send + Sync>),
    #[error("Load error: {0}")]
    Load(String),
    #[error("Start error: {0}")]
    Start(String),
}

pub type Result<T> = std::result::Result<T, Error>;
