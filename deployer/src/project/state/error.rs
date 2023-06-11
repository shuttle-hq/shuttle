use std::convert::Infallible;

use async_trait::async_trait;
use bollard::errors::Error as DockerError;
use shuttle_common::models::error::ErrorKind;
use tracing::{error, instrument};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to prepare the shuttle runtime: {0}")]
    RuntimePrepare(String),
}

use http::uri::InvalidUri;
use serde::{Deserialize, Serialize};

use crate::project::{docker::DockerContext, machine::State, service::Service};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectErrorKind {
    Internal,
    NoNetwork,
}

/// A runtime error coming from inside a project
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectError {
    kind: ProjectErrorKind,
    message: String,
    ctx: Option<Box<Service>>,
}

impl ProjectError {
    pub fn internal<S: AsRef<str>>(message: S) -> Self {
        Self {
            kind: ProjectErrorKind::Internal,
            message: message.as_ref().to_string(),
            ctx: None,
        }
    }

    pub fn no_network<S: AsRef<str>>(message: S) -> Self {
        Self {
            kind: ProjectErrorKind::NoNetwork,
            message: message.as_ref().to_string(),
            ctx: None,
        }
    }
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProjectError {}

impl From<DockerError> for ProjectError {
    fn from(err: DockerError) -> Self {
        tracing::error!(error = %err, "an internal DockerError had to yield a ProjectError");
        Self {
            kind: ProjectErrorKind::Internal,
            message: format!("{}", err),
            ctx: None,
        }
    }
}

impl From<InvalidUri> for ProjectError {
    fn from(uri: InvalidUri) -> Self {
        tracing::error!(%uri, "failed to create a health check URI");

        Self {
            kind: ProjectErrorKind::Internal,
            message: uri.to_string(),
            ctx: None,
        }
    }
}

impl From<hyper::Error> for ProjectError {
    fn from(err: hyper::Error) -> Self {
        error!(error = %err, "failed to check project's health");

        Self {
            kind: ProjectErrorKind::Internal,
            message: err.to_string(),
            ctx: None,
        }
    }
}

impl From<ProjectError> for Error {
    fn from(err: ProjectError) -> Self {
        Self::source(ErrorKind::Internal, err)
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectError
where
    Ctx: DockerContext,
{
    type Next = Self;
    type Error = Infallible;

    #[instrument(skip_all)]
    async fn next(self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}
