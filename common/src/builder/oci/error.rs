use super::image::digest::Digest;
use oci_spec::OciSpecError;
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
    #[error("No index.json is included in oci-archive: {0}")]
    MissingIndex(String),
    #[error("index.json does not have image name in manifest annotation")]
    MissingManifestName,
    #[error(transparent)]
    InvalidJson(#[from] serde_json::error::Error),

    //
    // Error from OCI registry
    //
    #[error("Reqwest error: {0}")]
    Reqwest(String),
    #[error("Authorization failed: {0}")]
    ChallengeFailed(String),
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
            // Runtime error when a `build()` (related to the oci-spec crate depdency on the `derive_builder` crate)
            // method is called and one or more required fields do not have a value.
            OciSpecError::Builder(e) => {
                panic!("Unknown oci-spec crate #[derive_builder] error: {}", e)
            }
            OciSpecError::Other(e) => panic!("Unknown error within oci_spec: {}", e),
        }
    }
}
