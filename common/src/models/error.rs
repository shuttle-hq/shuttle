use std::fmt::{Display, Formatter};

use http::StatusCode;
use serde::{Deserialize, Serialize};

#[cfg(feature = "display")]
use crossterm::style::Stylize;

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        tracing::warn!("{}", self.message);

        (self.status(), axum::Json(self)).into_response()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[typeshare::typeshare]
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

    /// Creates an internal error without exposing sensitive information to the user.
    #[inline(always)]
    #[allow(unused_variables)]
    pub fn internal_safe<E>(message: &str, error: E) -> Self
    where
        E: std::error::Error + 'static,
    {
        tracing::error!(error = &error as &dyn std::error::Error, "{message}");

        // Return the raw error during debug builds
        #[cfg(debug_assertions)]
        {
            ApiError::internal(&error.to_string())
        }
        // Return the safe message during release builds
        #[cfg(not(debug_assertions))]
        {
            ApiError::internal(message)
        }
    }

    pub fn unavailable(error: impl std::error::Error) -> Self {
        Self {
            message: error.to_string(),
            status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
        }
    }

    pub fn bad_request(error: impl std::error::Error) -> Self {
        Self {
            message: error.to_string(),
            status_code: StatusCode::BAD_REQUEST.as_u16(),
        }
    }

    pub fn unauthorized() -> Self {
        Self {
            message: "Unauthorized".to_string(),
            status_code: StatusCode::UNAUTHORIZED.as_u16(),
        }
    }

    pub fn forbidden() -> Self {
        Self {
            message: "Forbidden".to_string(),
            status_code: StatusCode::FORBIDDEN.as_u16(),
        }
    }

    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

pub trait ErrorContext<T> {
    /// Make a new internal server error with the given message.
    #[inline(always)]
    fn context_internal_error(self, message: &str) -> Result<T, ApiError>
    where
        Self: Sized,
    {
        self.with_context_internal_error(move || message.to_string())
    }

    /// Make a new internal server error using the given function to create the message.
    fn with_context_internal_error(self, message: impl FnOnce() -> String) -> Result<T, ApiError>;

    /// Make a new bad request error with the given message.
    #[inline(always)]
    fn context_bad_request(self, message: &str) -> Result<T, ApiError>
    where
        Self: Sized,
    {
        self.with_context_bad_request(move || message.to_string())
    }

    /// Make a new bad request error using the given function to create the message.
    fn with_context_bad_request(self, message: impl FnOnce() -> String) -> Result<T, ApiError>;

    /// Make a new not found error with the given message.
    #[inline(always)]
    fn context_not_found(self, message: &str) -> Result<T, ApiError>
    where
        Self: Sized,
    {
        self.with_context_not_found(move || message.to_string())
    }

    /// Make a new not found error using the given function to create the message.
    fn with_context_not_found(self, message: impl FnOnce() -> String) -> Result<T, ApiError>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: std::error::Error + 'static,
{
    #[inline(always)]
    fn with_context_internal_error(self, message: impl FnOnce() -> String) -> Result<T, ApiError> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(ApiError::internal_safe(message().as_ref(), error)),
        }
    }

    #[inline(always)]
    fn with_context_bad_request(self, message: impl FnOnce() -> String) -> Result<T, ApiError> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err({
                let message = message();
                tracing::warn!(
                    error = &error as &dyn std::error::Error,
                    "bad request: {message}"
                );

                ApiError {
                    message,
                    status_code: StatusCode::BAD_REQUEST.as_u16(),
                }
            }),
        }
    }

    #[inline(always)]
    fn with_context_not_found(self, message: impl FnOnce() -> String) -> Result<T, ApiError> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err({
                let message = message();
                tracing::warn!(
                    error = &error as &dyn std::error::Error,
                    "not found: {message}"
                );

                ApiError {
                    message,
                    status_code: StatusCode::NOT_FOUND.as_u16(),
                }
            }),
        }
    }
}

impl<T> ErrorContext<T> for Option<T> {
    #[inline]
    fn with_context_internal_error(self, message: impl FnOnce() -> String) -> Result<T, ApiError> {
        match self {
            Some(value) => Ok(value),
            None => Err(ApiError::internal(message().as_ref())),
        }
    }

    #[inline]
    fn with_context_bad_request(self, message: impl FnOnce() -> String) -> Result<T, ApiError> {
        match self {
            Some(value) => Ok(value),
            None => Err({
                ApiError {
                    message: message(),
                    status_code: StatusCode::BAD_REQUEST.as_u16(),
                }
            }),
        }
    }

    #[inline]
    fn with_context_not_found(self, message: impl FnOnce() -> String) -> Result<T, ApiError> {
        match self {
            Some(value) => Ok(value),
            None => Err({
                ApiError {
                    message: message(),
                    status_code: StatusCode::NOT_FOUND.as_u16(),
                }
            }),
        }
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "display")]
        return write!(
            f,
            "{}\nMessage: {}",
            self.status().to_string().bold(),
            self.message.to_string().red()
        );
        #[cfg(not(feature = "display"))]
        return write!(f, "{}\nMessage: {}", self.status(), self.message);
    }
}

impl std::error::Error for ApiError {}
