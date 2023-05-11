use super::digest::Digest;
use oci_spec::{distribution::ErrorResponse, OciSpecError};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    //
    // Invalid user input
    //
    #[error("Invalid digest: {0}")]
    InvalidDigest(String),
    #[error("Invalid name for repository: {0}")]
    InvalidName(String),
    #[error(transparent)]
    InvalidPort(#[from] std::num::ParseIntError),
    #[error("Invalid reference to image: {0}")]
    InvalidReference(String),
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
    #[error("Not a file, or not exist: {0}")]
    NotAFile(PathBuf),

    //
    // Invalid container image
    //
    #[error("Unknown digest in oci-archive: {0}")]
    UnknownDigest(Digest),
    #[error("No index.json is included in oci-archive")]
    MissingIndex,
    #[error("index.json does not have image name in manifest annotation")]
    MissingManifestName,
    #[error(transparent)]
    InvalidJson(#[from] serde_json::error::Error),

    //
    // Error from OCI registry
    //
    #[error("Network error: {0}")]
    Network(String),
    #[error("Registry error: {0}")]
    Registry(String),
    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),
    #[error("Unsupported WWW-Authentication header: {0}")]
    UnSupportedAuthHeader(String),

    //
    // System error
    //
    #[error(transparent)]
    UnknownIo(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<OciSpecError> for Error {
    fn from(e: OciSpecError) -> Self {
        match e {
            OciSpecError::SerDe(e) => Error::InvalidJson(e),
            OciSpecError::Io(e) => Error::UnknownIo(e),
            OciSpecError::Builder(_) => unreachable!(),
            OciSpecError::Other(e) => panic!("Unknown error within oci_spec: {}", e),
        }
    }
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(_status, res) => match res.into_json::<ErrorResponse>() {
                Ok(err) => Error::Registry(err.to_string()),
                Err(e) => Error::UnknownIo(e),
            },
            ureq::Error::Transport(e) => {
                Error::Network(e.message().expect("to have an error message").to_string())
            }
        }
    }
}
