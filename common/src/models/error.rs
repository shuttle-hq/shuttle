use std::fmt::{Display, Formatter};

use comfy_table::Color;
use crossterm::style::Stylize;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
pub enum ErrorKind {
    KeyMissing,
    BadHost,
    KeyMalformed,
    Unauthorized,
    Forbidden,
    UserNotFound,
    UserAlreadyExists,
    ProjectNotFound,
    InvalidProjectName,
    ProjectAlreadyExists,
    ProjectNotReady,
    ProjectUnavailable,
    CustomDomainNotFound,
    InvalidCustomDomain,
    CustomDomainAlreadyExists,
    InvalidOperation,
    Internal,
    NotReady,
    ServiceUnavailable,
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
                "project not found. Run `cargo shuttle project start` to create a new project.",
            ),
            ErrorKind::ProjectNotReady => (StatusCode::SERVICE_UNAVAILABLE, "project not ready"),
            ErrorKind::ProjectUnavailable => {
                (StatusCode::BAD_GATEWAY, "project returned invalid response")
            }
            ErrorKind::InvalidProjectName => (
                StatusCode::BAD_REQUEST,
                r#"
            Invalid project name. Project name must:
            1. start and end with alphanumeric characters.
            2. only contain lowercase characters.
            3. only contain characters inside of the alphanumeric range, except for `-`.
            4. not be empty.
            5. be shorter than 63 characters.
            6. not contain profanity.
            7. not be a reserved word."#,
            ),
            ErrorKind::InvalidOperation => (
                StatusCode::BAD_REQUEST,
                "the requested operation is invalid",
            ),
            ErrorKind::ProjectAlreadyExists => (
                StatusCode::BAD_REQUEST,
                "a project with the same name already exists",
            ),
            ErrorKind::InvalidCustomDomain => (StatusCode::BAD_REQUEST, "invalid custom domain"),
            ErrorKind::CustomDomainNotFound => (StatusCode::NOT_FOUND, "custom domain not found"),
            ErrorKind::CustomDomainAlreadyExists => {
                (StatusCode::BAD_REQUEST, "custom domain already in use")
            }
            ErrorKind::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ErrorKind::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ErrorKind::NotReady => (StatusCode::INTERNAL_SERVER_ERROR, "service not ready"),
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
                warn!("got a bad response from a deployer");
                "response from deployer is invalid. Please create a ticket to report this"
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
