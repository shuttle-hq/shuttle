use std::sync::Arc;

use crate::{error::Result, persistence::Service};
use async_trait::async_trait;
use axum::{
    extract::{FromRequest, Path},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use super::user::UserGuard;

/// Guard used to make sure a request has a valid api key set on the Basic Auth and that it owns a service
///
/// *Note*
/// This guard requires the [Arc<dyn ServiceAuthorizer>] extension to be set
pub struct ServiceGuard {
    pub id: Uuid,
    pub name: String,
}

#[async_trait]
impl<B> FromRequest<B> for ServiceGuard
where
    B: Send,
{
    type Rejection = (StatusCode, Json<ServiceGuardError>);

    async fn from_request(
        req: &mut axum::extract::RequestParts<B>,
    ) -> std::result::Result<Self, Self::Rejection> {
        let user_guard = req.extract::<UserGuard>().await.map_err(|e| {
            (
                e.0,
                Json(ServiceGuardError {
                    message: e.1.message.to_string(),
                }),
            )
        })?;
        let Path(service_name) = req.extract::<Path<String>>().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ServiceGuardError {
                    message: e.to_string(),
                }),
            )
        })?;

        let user_authorizer = req
            .extensions()
            .get::<Arc<dyn ServiceAuthorizer>>()
            .expect("Arc<dyn ServiceAuthorizer> to be available on extensions");

        if let Some(service) = user_authorizer
            .does_user_own_service(&user_guard.api_key, &service_name)
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ServiceGuardError {
                        message: e.to_string(),
                    }),
                )
            })?
        {
            Ok(service.into())
        } else {
            Err((
                StatusCode::FORBIDDEN,
                Json(ServiceGuardError {
                    message: "request could not be authorized".to_string(),
                }),
            ))
        }
    }
}

#[derive(Serialize)]
pub struct ServiceGuardError {
    pub message: String,
}

#[async_trait::async_trait]
pub trait ServiceAuthorizer: Sync + Send {
    async fn does_user_own_service(
        &self,
        api_key: &str,
        service_name: &str,
    ) -> Result<Option<Service>>;
}

impl From<Service> for ServiceGuard {
    fn from(service: Service) -> Self {
        Self {
            id: service.id,
            name: service.name,
        }
    }
}
