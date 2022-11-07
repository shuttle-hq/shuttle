use std::fmt::{Display, Formatter};

use comfy_table::Color;
use crossterm::style::Stylize;
use http::StatusCode;
use serde::{Deserialize, Serialize};

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
}

impl From<ErrorKind> for ApiError {
    fn from(kind: ErrorKind) -> Self {
        let (status, error_message) = match kind {
            ErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
            ErrorKind::KeyMissing => (StatusCode::UNAUTHORIZED, "request is missing a key"),
            ErrorKind::KeyMalformed => (StatusCode::BAD_REQUEST, "request has an invalid key"),
            ErrorKind::BadHost => (StatusCode::BAD_REQUEST, "the 'Host' header is invalid"),
            ErrorKind::UserNotFound => (StatusCode::NOT_FOUND, "user not found"),
            ErrorKind::UserAlreadyExists => (StatusCode::BAD_REQUEST, "user already exists"),
            ErrorKind::ProjectNotFound => (
                StatusCode::NOT_FOUND,
                "project not found. Run `cargo shuttle project new` to create a new project.",
            ),
            ErrorKind::ProjectNotReady => (StatusCode::SERVICE_UNAVAILABLE, "project not ready"),
            ErrorKind::ProjectUnavailable => {
                (StatusCode::BAD_GATEWAY, "project returned invalid response")
            }
            ErrorKind::InvalidProjectName => (StatusCode::BAD_REQUEST, "invalid project name"),
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
