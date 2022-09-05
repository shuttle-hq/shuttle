use std::sync::Arc;

use crate::{error::Result, persistence::User};
use async_trait::async_trait;
use axum::{
    extract::FromRequest,
    headers::{authorization::Basic, Authorization},
    http::StatusCode,
    Json, TypedHeader,
};
use serde::Serialize;
use tracing::Span;

/// Guard used to make sure a request has a valid api key set on the Basic Auth
///
/// *Note*
/// This guard requires the [Arc<dyn UserValidator>] extension to be set
pub struct UserGuard {
    pub api_key: String,
}

#[async_trait]
impl<B> FromRequest<B> for UserGuard
where
    B: Send,
{
    type Rejection = (StatusCode, Json<UserGuardError>);

    async fn from_request(
        req: &mut axum::extract::RequestParts<B>,
    ) -> std::result::Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(basic)) =
            TypedHeader::<Authorization<Basic>>::from_request(req)
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(UserGuardError {
                            message: e.to_string(),
                        }),
                    )
                })?;
        let user_validator = req
            .extensions()
            .get::<Arc<dyn UserValidator>>()
            .expect("Arc<dyn UserValidator> to be available on extensions");

        if let Some(user) = user_validator
            .is_user_valid(basic.username())
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(UserGuardError {
                        message: e.to_string(),
                    }),
                )
            })?
        {
            // Record api_key for tracing purposes
            Span::current().record("api_key", &user.api_key);
            Ok(user.into())
        } else {
            Err((
                StatusCode::FORBIDDEN,
                Json(UserGuardError {
                    message: "request could not be authenticated".to_string(),
                }),
            ))
        }
    }
}

#[derive(Serialize)]
pub struct UserGuardError {
    pub message: String,
}

#[async_trait::async_trait]
pub trait UserValidator: Sync + Send {
    async fn is_user_valid(&self, api_key: &str) -> Result<Option<User>>;
}

impl From<User> for UserGuard {
    fn from(user: User) -> Self {
        Self {
            api_key: user.api_key,
        }
    }
}
