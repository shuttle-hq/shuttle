use std::sync::Arc;

use crate::{
    error::Result,
    persistence::{Deployment, State},
};
use async_trait::async_trait;
use axum::{
    extract::{FromRequest, Path},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::user::UserGuard;

/// Guard used to make sure a request has a valid api key set on the Basic Auth and that it owns a service's deployment
///
/// *Note*
/// This guard requires the [Arc<dyn DeploymentAuthorizer>] extension to be set
pub struct DeploymentGuard {
    pub id: Uuid,
    pub service_id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
}

#[async_trait]
impl<B> FromRequest<B> for DeploymentGuard
where
    B: Send,
{
    type Rejection = (StatusCode, Json<DeploymentGuardError>);

    async fn from_request(
        req: &mut axum::extract::RequestParts<B>,
    ) -> std::result::Result<Self, Self::Rejection> {
        let user_guard = req.extract::<UserGuard>().await.map_err(|e| {
            (
                e.0,
                Json(DeploymentGuardError {
                    message: e.1.message.to_string(),
                }),
            )
        })?;
        let Path(deployment_id) = req.extract::<Path<Uuid>>().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(DeploymentGuardError {
                    message: e.to_string(),
                }),
            )
        })?;

        let deployment_authorizer = req
            .extensions()
            .get::<Arc<dyn DeploymentAuthorizer>>()
            .expect("Arc<dyn DeploymentAuthorizer> to be available on extensions");

        if let Some(deployment) = deployment_authorizer
            .does_user_own_deployment(&user_guard.api_key, &deployment_id)
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(DeploymentGuardError {
                        message: e.to_string(),
                    }),
                )
            })?
        {
            Ok(deployment.into())
        } else {
            Err((
                StatusCode::FORBIDDEN,
                Json(DeploymentGuardError {
                    message: "request could not be authorized".to_string(),
                }),
            ))
        }
    }
}

#[derive(Serialize)]
pub struct DeploymentGuardError {
    pub message: String,
}

#[async_trait::async_trait]
pub trait DeploymentAuthorizer: Sync + Send {
    async fn does_user_own_deployment(
        &self,
        api_key: &str,
        deployment_id: &Uuid,
    ) -> Result<Option<Deployment>>;
}

impl From<Deployment> for DeploymentGuard {
    fn from(deployment: Deployment) -> Self {
        Self {
            id: deployment.id,
            service_id: deployment.service_id,
            state: deployment.state,
            last_update: deployment.last_update,
        }
    }
}

impl From<DeploymentGuard> for shuttle_common::deployment::Response {
    fn from(deployment: DeploymentGuard) -> Self {
        Self {
            id: deployment.id,
            service_id: deployment.service_id,
            state: deployment.state.into(),
            last_update: deployment.last_update,
        }
    }
}
