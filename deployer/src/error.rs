use shuttle_common::models::error::ErrorKind;
use std::{error::Error as StdError, fmt::Formatter, io};

/// [`Error] is safe to be used as error variants to axum endpoints
/// return types as their [`IntoResponse`] implementation does not
/// leak any sensitive information.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Sync + Send + 'static>>,
}

impl Error {
    pub fn source<E: StdError + Sync + Send + 'static>(kind: ErrorKind, err: E) -> Self {
        Self {
            kind,
            source: Some(Box::new(err)),
        }
    }

    pub fn custom<S: AsRef<str>>(kind: ErrorKind, message: S) -> Self {
        Self {
            kind,
            source: Some(Box::new(io::Error::new(
                io::ErrorKind::Other,
                message.as_ref().to_string(),
            ))),
        }
    }

    pub fn from_kind(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::from_kind(kind)
    }
}

impl From<io::Error> for Error {
    fn from(_: io::Error) -> Self {
        Self::from(ErrorKind::Internal)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(source) = self.source.as_ref() {
            write!(f, ": ")?;
            source.fmt(f)?;
        }
        Ok(())
    }
}

impl StdError for Error {}
