use std::convert::Infallible;

use async_trait::async_trait;
use bollard::errors::Error as DockerError;
use http::uri::InvalidUri;
use serde::{Deserialize, Serialize};
use tracing::{error, instrument};

use super::machine::State;
use crate::project::{docker::DockerContext, service::ServiceState};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceErroredKind {
    Internal,
    NoNetwork,
}

/// A runtime error coming from inside a project
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceErrored {
    kind: ServiceErroredKind,
    message: String,
    pub ctx: Option<Box<ServiceState>>,
}

impl ServiceErrored {
    pub fn internal<S: AsRef<str>>(message: S) -> Self {
        Self {
            kind: ServiceErroredKind::Internal,
            message: message.as_ref().to_string(),
            ctx: None,
        }
    }

    pub fn no_network<S: AsRef<str>>(message: S) -> Self {
        Self {
            kind: ServiceErroredKind::NoNetwork,
            message: message.as_ref().to_string(),
            ctx: None,
        }
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceErrored
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

impl std::fmt::Display for ServiceErrored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ServiceErrored {}

impl From<DockerError> for ServiceErrored {
    fn from(err: DockerError) -> Self {
        tracing::error!(error = %err, "an internal DockerError had to yield a ProjectError");
        Self {
            kind: ServiceErroredKind::Internal,
            message: format!("{}", err),
            ctx: None,
        }
    }
}

impl From<InvalidUri> for ServiceErrored {
    fn from(uri: InvalidUri) -> Self {
        tracing::error!(%uri, "failed to create a health check URI");

        Self {
            kind: ServiceErroredKind::Internal,
            message: uri.to_string(),
            ctx: None,
        }
    }
}

impl From<hyper::Error> for ServiceErrored {
    fn from(err: hyper::Error) -> Self {
        error!(error = %err, "failed to check project's health");

        Self {
            kind: ServiceErroredKind::Internal,
            message: err.to_string(),
            ctx: None,
        }
    }
}
