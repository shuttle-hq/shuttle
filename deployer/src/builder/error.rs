use std::io;

use super::oci;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OCI image: {0}")]
    Oci(oci::error::Error),
    #[error("StdIo error: {0}")]
    Io(io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<oci::error::Error> for Error {
    fn from(e: oci::error::Error) -> Self {
        Error::Oci(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
