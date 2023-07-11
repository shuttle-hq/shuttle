use std::io;

use super::oci;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OCI image: {0}")]
    Oci(#[from] oci::error::Error),
    #[error("StdIo error: {0}")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
