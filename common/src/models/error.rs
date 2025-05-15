use std::fmt::Display;

use http::{status::InvalidStatusCode, StatusCode};
use serde::{Deserialize, Serialize};

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        #[cfg(feature = "tracing-in-errors")]
        tracing::warn!("{}", self.message);

        (
            self.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            axum::Json(self),
        )
            .into_response()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ApiError {
    message: String,
    status_code: u16,
}

impl ApiError {
    #[inline(always)]
    pub fn new(message: impl Display, status_code: StatusCode) -> Self {
        Self {
            message: message.to_string(),
            status_code: status_code.as_u16(),
        }
    }
    #[inline(always)]
    pub fn status(&self) -> Result<StatusCode, InvalidStatusCode> {
        StatusCode::from_u16(self.status_code)
    }
    #[inline(always)]
    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    /// Create a one-off internal error with a string message exposed to the user.
    #[inline(always)]
    pub fn internal(message: impl AsRef<str>) -> Self {
        #[cfg(feature = "tracing-in-errors")]
        {
            /// Dummy wrapper to allow logging a string `as &dyn std::error::Error`
            #[derive(Debug)]
            struct InternalError(String);
            impl std::error::Error for InternalError {}
            impl std::fmt::Display for InternalError {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(self.0.as_str())
                }
            }

            tracing::error!(
                error = &InternalError(message.as_ref().to_owned()) as &dyn std::error::Error,
                "Internal API Error"
            );
        }

        Self::_internal(message.as_ref())
    }

    /// Creates an internal error without exposing sensitive information to the user.
    #[inline(always)]
    #[allow(unused_variables)]
    pub fn internal_safe<E: std::error::Error + 'static>(safe_msg: impl Display, error: E) -> Self {
        #[cfg(feature = "tracing-in-errors")]
        tracing::error!(error = &error as &dyn std::error::Error, "{}", safe_msg);

        // Return the raw error during debug builds
        #[cfg(debug_assertions)]
        {
            Self::_internal(error)
        }
        // Return the safe message during release builds
        #[cfg(not(debug_assertions))]
        {
            Self::_internal(safe_msg)
        }
    }

    // 5xx
    #[inline(always)]
    fn _internal(error: impl Display) -> Self {
        Self::new(error.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
    }
    #[inline(always)]
    pub fn service_unavailable(error: impl Display) -> Self {
        Self::new(error.to_string(), StatusCode::SERVICE_UNAVAILABLE)
    }
    // 4xx
    #[inline(always)]
    pub fn bad_request(error: impl Display) -> Self {
        Self::new(error.to_string(), StatusCode::BAD_REQUEST)
    }
    #[inline(always)]
    pub fn unauthorized(error: impl Display) -> Self {
        Self::new(error.to_string(), StatusCode::UNAUTHORIZED)
    }
    #[inline(always)]
    pub fn forbidden(error: impl Display) -> Self {
        Self::new(error.to_string(), StatusCode::FORBIDDEN)
    }
    #[inline(always)]
    pub fn not_found(error: impl Display) -> Self {
        Self::new(error.to_string(), StatusCode::NOT_FOUND)
    }
}

pub trait ErrorContext<T> {
    /// Make a new internal server error with the given message.
    fn context_internal_error(self, message: impl Display) -> Result<T, ApiError>;

    /// Make a new internal server error using the given function to create the message.
    #[inline(always)]
    fn with_context_internal_error(self, message: impl FnOnce() -> String) -> Result<T, ApiError>
    where
        Self: Sized,
    {
        self.context_internal_error(message())
    }
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: std::error::Error + 'static,
{
    #[inline(always)]
    fn context_internal_error(self, message: impl Display) -> Result<T, ApiError> {
        self.map_err(|error| ApiError::internal_safe(message, error))
    }
}

impl std::fmt::Display for ApiError {
    #[cfg(feature = "display")]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crossterm::style::Stylize;
        write!(
            f,
            "{}\nMessage: {}",
            self.status()
                .map(|s| s.to_string())
                .unwrap_or("Unknown".to_owned())
                .bold(),
            self.message.to_string().red()
        )
    }
    #[cfg(not(feature = "display"))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\nMessage: {}",
            self.status()
                .map(|s| s.to_string())
                .unwrap_or("Unknown".to_owned()),
            self.message,
        )
    }
}

impl std::error::Error for ApiError {}
