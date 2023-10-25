use std::fmt::{Display, Formatter};

use crossterm::style::{Color, Stylize};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::project::InvalidProjectName;

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
    pub status_code: u16,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\nmessage: {}",
            self.status().to_string().bold(),
            self.message.to_string().with(Color::Red)
        )
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Clone, PartialEq, strum::Display)]
pub enum ErrorKind {
    KeyMissing,
    BadHost,
    KeyMalformed,
    Unauthorized,
    Forbidden,
    UserNotFound,
    UserAlreadyExists,
    ProjectNotFound,
    InvalidProjectName(InvalidProjectName),
    ProjectAlreadyExists,
    /// Contains a message describing a running state of the project.
    /// Used if the project already exists but is owned
    /// by the caller, which means they can modify the project.
    OwnProjectAlreadyExists(String),
    ProjectNotReady,
    ProjectUnavailable,
    ProjectHasResources(Vec<String>),
    ProjectHasRunningDeployment,
    CustomDomainNotFound,
    InvalidCustomDomain,
    CustomDomainAlreadyExists,
    InvalidOperation,
    Internal,
    NotReady,
    ServiceUnavailable,
    DeleteProjectFailed,
}

impl From<ErrorKind> for ApiError {
    fn from(kind: ErrorKind) -> Self {
        let (status, error_message) = match kind {
            ErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
            ErrorKind::KeyMissing => (StatusCode::UNAUTHORIZED, "request is missing a key"),
            ErrorKind::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "we're experiencing a high workload right now, please try again in a little bit",
            ),
            ErrorKind::KeyMalformed => (StatusCode::BAD_REQUEST, "request has an invalid key"),
            ErrorKind::BadHost => (StatusCode::BAD_REQUEST, "the 'Host' header is invalid"),
            ErrorKind::UserNotFound => (StatusCode::NOT_FOUND, "user not found"),
            ErrorKind::UserAlreadyExists => (StatusCode::BAD_REQUEST, "user already exists"),
            ErrorKind::ProjectNotFound => (
                StatusCode::NOT_FOUND,
                "project not found. Make sure you are the owner of this project name. Run `cargo shuttle project start` to create a new project.",
            ),
            ErrorKind::ProjectNotReady => (
                StatusCode::SERVICE_UNAVAILABLE,
                "project not ready. Try running `cargo shuttle project restart`.",
            ),
            ErrorKind::ProjectUnavailable => (StatusCode::BAD_GATEWAY, "project returned invalid response"),
            ErrorKind::ProjectHasRunningDeployment => (
                StatusCode::FORBIDDEN,
                "A deployment is running. Stop it with `cargo shuttle stop` first."
            ),
            ErrorKind::ProjectHasResources(resources) => {
                let resources = resources.join(", ");
                return Self {
                    message: format!("Project has resources: {}. Use `cargo shuttle resource list` and `cargo shuttle resource delete <type>` to delete them.", resources),
                    status_code: StatusCode::FORBIDDEN.as_u16(),
                }
            }
            ErrorKind::InvalidProjectName(err) => {
                return Self {
                    message: err.to_string(),
                    status_code: StatusCode::BAD_REQUEST.as_u16(),
                }
            }
            ErrorKind::InvalidOperation => (StatusCode::BAD_REQUEST, "the requested operation is invalid"),
            ErrorKind::ProjectAlreadyExists => (StatusCode::BAD_REQUEST, "a project with the same name already exists"),
            ErrorKind::OwnProjectAlreadyExists(message) => {
                return Self {
                    message,
                    status_code: StatusCode::BAD_REQUEST.as_u16(),
                }
            }
            ErrorKind::InvalidCustomDomain => (StatusCode::BAD_REQUEST, "invalid custom domain"),
            ErrorKind::CustomDomainNotFound => (StatusCode::NOT_FOUND, "custom domain not found"),
            ErrorKind::CustomDomainAlreadyExists => (StatusCode::BAD_REQUEST, "custom domain already in use"),
            ErrorKind::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ErrorKind::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ErrorKind::NotReady => (StatusCode::INTERNAL_SERVER_ERROR, "service not ready"),
            ErrorKind::DeleteProjectFailed => (StatusCode::INTERNAL_SERVER_ERROR, "deleting project failed"),
        };
        Self {
            message: error_message.to_string(),
            status_code: status.as_u16(),
        }
    }
}

impl From<StatusCode> for ApiError {
    fn from(code: StatusCode) -> Self {
        let message = match code {
            StatusCode::OK | StatusCode::ACCEPTED | StatusCode::FOUND | StatusCode::SWITCHING_PROTOCOLS => {
                unreachable!("we should not have an API error with a successful status code")
            }
            StatusCode::FORBIDDEN => "this request is not allowed",
            StatusCode::UNAUTHORIZED => {
                "we were unable to authorize your request. Is your key still valid?"
            },
            StatusCode::INTERNAL_SERVER_ERROR => "our server was unable to handle your request. A ticket should be created for us to fix this.",
            StatusCode::SERVICE_UNAVAILABLE => "we're experiencing a high workload right now, please try again in a little bit",
            StatusCode::BAD_REQUEST => {
                warn!("responding to a BAD_REQUEST request with an unhelpful message. Use ErrorKind instead");
                "this request is invalid"
            },
            StatusCode::NOT_FOUND => {
                warn!("responding to a NOT_FOUND request with an unhelpful message. Use ErrorKind instead");
                "we don't serve this resource"
            },
            StatusCode::BAD_GATEWAY => {
                warn!("got a bad response from the gateway");
                // Gateway's default response when a request handler panicks is a 502 with some HTML.
                "response from gateway is invalid. Please create a ticket to report this"
            },
            _ => {
                error!(%code, "got an unexpected status code");
                "an unexpected error occurred. Please create a ticket to report this"
            },
        };

        Self {
            message: message.to_string(),
            status_code: code.as_u16(),
        }
    }
}
