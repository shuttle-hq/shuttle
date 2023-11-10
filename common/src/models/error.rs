use std::fmt::{Display, Formatter};

use crossterm::style::{Color, Stylize};
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
            "{}\nMessage: {}",
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
    TooManyProjects,
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
                // "not ready" is matched against in cargo-shuttle for giving further instructions on project deletion
                "project not ready. Try running `cargo shuttle project restart`.",
            ),
            ErrorKind::ProjectUnavailable => {
                (StatusCode::BAD_GATEWAY, "project returned invalid response")
            },
            ErrorKind::TooManyProjects => {
                (StatusCode::FORBIDDEN, "You cannot create more projects. Delete some projects first.")
            },
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

// Used as a fallback when an API response did not contain a serialized ApiError
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

#[cfg(feature = "backend")]
pub mod axum {
    use async_trait::async_trait;
    use axum::extract::path::ErrorKind;
    use axum::{
        extract::{rejection::PathRejection, FromRequestParts},
        http::request::Parts,
        response::{IntoResponse, Json, Response},
    };
    use http::StatusCode;
    use serde::de::DeserializeOwned;

    use super::ApiError;

    impl IntoResponse for ApiError {
        fn into_response(self) -> Response {
            (self.status(), Json(self)).into_response()
        }
    }

    /// Custom `Path` extractor that customizes the error from `axum::extract::Path`.
    ///
    /// Prints the custom error message if deserialization resulted in a custom de::Error,
    /// which is what the [`shuttle_common::project::ProjectName`] parser uses.
    pub struct CustomErrorPath<T>(pub T);

    impl<T> core::ops::Deref for CustomErrorPath<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T> core::ops::DerefMut for CustomErrorPath<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    #[async_trait]
    impl<S, T> FromRequestParts<S> for CustomErrorPath<T>
    where
        T: DeserializeOwned + Send,
        S: Send + Sync,
    {
        type Rejection = ApiError;

        async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
            match axum::extract::Path::<T>::from_request_parts(parts, state).await {
                Ok(value) => Ok(Self(value.0)),
                Err(rejection) => {
                    if let PathRejection::FailedToDeserializePathParams(inner) = &rejection {
                        if let ErrorKind::Message(message) = inner.kind() {
                            return Err(ApiError {
                                message: message.clone(),
                                status_code: StatusCode::BAD_REQUEST.as_u16(),
                            });
                        }
                    }

                    Err(ApiError {
                        message: rejection.body_text(),
                        status_code: rejection.status().as_u16(),
                    })
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::models::project::ProjectName;

        use super::*;
        use axum::http::StatusCode;
        use axum::{body::Body, routing::get, Router};
        use http::Request;
        use tower::Service;

        #[tokio::test]
        async fn project_name_paths() {
            let mut app = Router::new()
                .route(
                    "/:project_name",
                    get(
                        |CustomErrorPath(project_name): CustomErrorPath<ProjectName>| async move {
                            project_name.to_string()
                        },
                    ),
                )
                .route(
                    "/:project_name/:num",
                    get(
                        |CustomErrorPath((project_name, num)): CustomErrorPath<(
                            ProjectName,
                            u8,
                        )>| async move { format!("{project_name} {num}") },
                    ),
                );

            let response = app
                .call(Request::get("/test123").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
            assert_eq!(&body[..], b"test123");

            let response = app
                .call(Request::get("/__test123").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
            assert!(&body[..].starts_with(br#"{"message":"Invalid project name"#));

            let response = app
                .call(Request::get("/test123/123").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
            assert_eq!(&body[..], b"test123 123");

            let response = app
                .call(Request::get("/test123/asdf").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
            assert!(&body[..].starts_with(br#"{"message":"Invalid URL"#));
        }
    }
}
