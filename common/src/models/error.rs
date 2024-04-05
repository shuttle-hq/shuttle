use std::fmt::{Display, Formatter};

use crossterm::style::Stylize;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, warn};

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status(), axum::Json(self)).into_response()
    }
}

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
            "{}\nMessage: {}",
            self.status().to_string().bold(),
            self.message.to_string().red()
        )
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ErrorKind {
    #[error("Request is missing a key")]
    KeyMissing,
    #[error("The 'Host' header is invalid")]
    BadHost,
    #[error("Request has an invalid key")]
    KeyMalformed,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden")]
    Forbidden,
    #[error("User not found")]
    UserNotFound,
    #[error("User already exists")]
    UserAlreadyExists,
    #[error("Project '{0}' not found. Make sure you are the owner of this project name. Run `cargo shuttle project start` to create a new project.")]
    ProjectNotFound(String),
    #[error("{0:?}")]
    InvalidProjectName(InvalidProjectName),
    #[error("A project with the same name already exists")]
    ProjectAlreadyExists,
    /// Contains a message describing a running state of the project.
    /// Used if the project already exists but is owned
    /// by the caller, which means they can modify the project.
    #[error("{0}")]
    OwnProjectAlreadyExists(String),
    // "not ready" is matched against in cargo-shuttle for giving further instructions on project deletion
    #[error("Project not ready. Try running `cargo shuttle project restart`.")]
    ProjectNotReady,
    #[error("Project returned invalid response")]
    ProjectUnavailable,
    #[error("You cannot create more projects. Delete some projects first.")]
    TooManyProjects,
    #[error("Could not automatically delete the following resources: {0:?}. Please reach out to Shuttle support for help.")]
    ProjectHasResources(Vec<String>),
    #[error("Could not automatically stop the running deployment for the project. Please reach out to Shuttle support for help.")]
    ProjectHasRunningDeployment,
    #[error("Project currently has a deployment that is busy building. Use `cargo shuttle deployment list` to see it and wait for it to finish")]
    ProjectHasBuildingDeployment,
    #[error("Tried to get project into a ready state for deletion but failed. Please reach out to Shuttle support for help.")]
    ProjectCorrupted,
    #[error("Custom domain not found")]
    CustomDomainNotFound,
    #[error("Invalid custom domain")]
    InvalidCustomDomain,
    #[error("Custom domain already in use")]
    CustomDomainAlreadyExists,
    #[error("The requested operation is invalid")]
    InvalidOperation,
    #[error("Internal server error")]
    Internal,
    #[error("Service not ready")]
    NotReady,
    #[error("We're experiencing a high workload right now, please try again in a little bit")]
    ServiceUnavailable,
    #[error("Deleting project failed")]
    DeleteProjectFailed,
    #[error("Our server is at capacity and cannot serve your request at this time. Please try again in a few minutes.")]
    CapacityLimit,
    #[error("{0:?}")]
    InvalidOrganizationName(InvalidOrganizationName),
}

impl From<ErrorKind> for ApiError {
    fn from(kind: ErrorKind) -> Self {
        let status = match kind {
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::KeyMissing => StatusCode::UNAUTHORIZED,
            ErrorKind::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorKind::KeyMalformed => StatusCode::BAD_REQUEST,
            ErrorKind::BadHost => StatusCode::BAD_REQUEST,
            ErrorKind::UserNotFound => StatusCode::NOT_FOUND,
            ErrorKind::UserAlreadyExists => StatusCode::BAD_REQUEST,
            ErrorKind::ProjectNotFound(_) => StatusCode::NOT_FOUND,
            ErrorKind::ProjectNotReady => StatusCode::SERVICE_UNAVAILABLE,
            ErrorKind::ProjectUnavailable => StatusCode::BAD_GATEWAY,
            ErrorKind::TooManyProjects => StatusCode::FORBIDDEN,
            ErrorKind::ProjectHasRunningDeployment => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::ProjectHasBuildingDeployment => StatusCode::BAD_REQUEST,
            ErrorKind::ProjectCorrupted => StatusCode::BAD_REQUEST,
            ErrorKind::ProjectHasResources(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::InvalidProjectName(_) => StatusCode::BAD_REQUEST,
            ErrorKind::InvalidOperation => StatusCode::BAD_REQUEST,
            ErrorKind::ProjectAlreadyExists => StatusCode::BAD_REQUEST,
            ErrorKind::OwnProjectAlreadyExists(_) => StatusCode::BAD_REQUEST,
            ErrorKind::InvalidCustomDomain => StatusCode::BAD_REQUEST,
            ErrorKind::CustomDomainNotFound => StatusCode::NOT_FOUND,
            ErrorKind::CustomDomainAlreadyExists => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::NotReady => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::DeleteProjectFailed => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::CapacityLimit => StatusCode::SERVICE_UNAVAILABLE,
            ErrorKind::InvalidOrganizationName(_) => StatusCode::BAD_REQUEST,
        };
        Self {
            message: kind.to_string(),
            status_code: status.as_u16(),
        }
    }
}

// Used as a fallback when an API response did not contain a serialized ApiError
impl From<StatusCode> for ApiError {
    fn from(code: StatusCode) -> Self {
        let message = match code {
            StatusCode::OK | StatusCode::ACCEPTED | StatusCode::FOUND | StatusCode::SWITCHING_PROTOCOLS => {
                unreachable!("we should not have an API error with a successful status code")
            }
            StatusCode::FORBIDDEN => "This request is not allowed",
            StatusCode::UNAUTHORIZED => {
                "we were unable to authorize your request. Check that your API key is set correctly. Use `cargo shuttle login` to set it."
            },
            StatusCode::INTERNAL_SERVER_ERROR => "Our server was unable to handle your request. A ticket should be created for us to fix this.",
            StatusCode::SERVICE_UNAVAILABLE => "We're experiencing a high workload right now, please try again in a little bit",
            StatusCode::BAD_REQUEST => {
                warn!("responding to a BAD_REQUEST request with an unhelpful message. Use ErrorKind instead");
                "This request is invalid"
            },
            StatusCode::NOT_FOUND => {
                warn!("responding to a NOT_FOUND request with an unhelpful message. Use ErrorKind instead");
                "We don't serve this resource"
            },
            StatusCode::BAD_GATEWAY => {
                warn!("got a bad response from the gateway");
                // Gateway's default response when a request handler panicks is a 502 with some HTML.
                "Response from gateway is invalid. Please create a ticket to report this"
            },
            _ => {
                error!(%code, "got an unexpected status code");
                "An unexpected error occurred. Please create a ticket to report this"
            },
        };

        Self {
            message: message.to_string(),
            status_code: code.as_u16(),
        }
    }
}

// Note: The string "Invalid project name" is used by cargo-shuttle to determine what type of error was returned.
// Changing it is breaking.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error(
    "Invalid project name. Project names must:
    1. only contain lowercase alphanumeric characters or dashes `-`.
    2. not start or end with a dash.
    3. not be empty.
    4. be shorter than 64 characters.
    5. not contain any profanities.
    6. not be a reserved word."
)]
pub struct InvalidProjectName;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("Invalid organization name. Must not be more than 30 characters long.")]
pub struct InvalidOrganizationName;
