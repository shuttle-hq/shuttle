use std::fmt::{Display, Formatter};

use crossterm::style::Stylize;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, warn};

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        warn!("{}", self.message);

        (self.status(), axum::Json(self)).into_response()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
    pub status_code: u16,
}

impl ApiError {
    pub fn internal(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        }
    }

    pub fn unavailable(error: impl std::error::Error) -> Self {
        Self {
            message: error.to_string(),
            status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
        }
    }

    fn bad_request(error: impl std::error::Error) -> Self {
        Self {
            message: error.to_string(),
            status_code: StatusCode::BAD_REQUEST.as_u16(),
        }
    }

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

// Note: The string "Invalid project name" is used by cargo-shuttle to determine what type of error was returned.
// Changing it is breaking.
#[derive(Debug, Clone, PartialEq, Error)]
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

impl From<InvalidProjectName> for ApiError {
    fn from(err: InvalidProjectName) -> Self {
        Self::bad_request(err)
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
#[error("Invalid organization name. Must not be more than 30 characters long.")]
pub struct InvalidOrganizationName;

impl From<InvalidOrganizationName> for ApiError {
    fn from(err: InvalidOrganizationName) -> Self {
        Self::bad_request(err)
    }
}

#[derive(Debug, Error)]
#[error("Project is not ready. Try to restart it")]
pub struct ProjectNotReady;

#[derive(Debug, Error)]
#[error("Project is running but is not responding correctly. Try to restart it")]
pub struct ProjectUnavailable;

#[derive(Debug, Error)]
#[error("Project '{0}' not found. Make sure you are the owner of this project name. Run `cargo shuttle project start` to create a new project.")]
pub struct ProjectNotFound(pub String);

impl From<ProjectNotFound> for ApiError {
    fn from(err: ProjectNotFound) -> Self {
        Self {
            message: err.to_string(),
            status_code: StatusCode::NOT_FOUND.as_u16(),
        }
    }
}

#[derive(Debug, Error)]
#[error("Could not automatically delete the following resources: {0:?}. Please reach out to Shuttle support for help.")]
pub struct ProjectHasResources(pub Vec<String>);

impl From<ProjectHasResources> for ApiError {
    fn from(err: ProjectHasResources) -> Self {
        Self::bad_request(err)
    }
}

#[derive(Debug, Error)]
#[error("Could not automatically stop the running deployment for the project. Please reach out to Shuttle support for help.")]
pub struct ProjectHasRunningDeployment;

impl From<ProjectHasRunningDeployment> for ApiError {
    fn from(err: ProjectHasRunningDeployment) -> Self {
        Self::bad_request(err)
    }
}

#[derive(Debug, Error)]
#[error("Project currently has a deployment that is busy building. Use `cargo shuttle deployment list` to see it and wait for it to finish")]
pub struct ProjectHasBuildingDeployment;

impl From<ProjectHasBuildingDeployment> for ApiError {
    fn from(err: ProjectHasBuildingDeployment) -> Self {
        Self::bad_request(err)
    }
}

#[derive(Debug, Error)]
#[error("Tried to get project into a ready state for deletion but failed. Please reach out to Shuttle support for help.")]
pub struct ProjectCorrupted;

impl From<ProjectCorrupted> for ApiError {
    fn from(err: ProjectCorrupted) -> Self {
        Self::bad_request(err)
    }
}

#[derive(Debug, Error)]
#[error("Invalid custom domain")]
pub struct InvalidCustomDomain;

impl From<InvalidCustomDomain> for ApiError {
    fn from(err: InvalidCustomDomain) -> Self {
        Self::bad_request(err)
    }
}
