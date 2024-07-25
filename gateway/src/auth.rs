use axum::extract::{FromRef, FromRequestParts, Path};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use shuttle_backends::client::permit;
use shuttle_backends::project_name::ProjectName;
use shuttle_backends::ClaimExt;
use shuttle_common::claims::Claim;
use shuttle_common::models::error::{ApiError, InvalidProjectName, ProjectNotFound};
use thiserror::Error;
use tracing::error;
use ulid::Ulid;

use crate::api::latest::RouterState;
use crate::service;

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not serve your request")]
    StatusCode(StatusCode),

    #[error(transparent)]
    InvalidProjectName(#[from] InvalidProjectName),

    #[error(transparent)]
    ProjectNotFound(#[from] ProjectNotFound),

    #[error(transparent)]
    Permission(#[from] permit::Error),

    #[error(transparent)]
    Service(#[from] service::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let api_error: ApiError = self.into();

        api_error.into_response()
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            Error::StatusCode(e) => Self {
                message: e.to_string(),
                status_code: e.as_u16(),
            },
            Error::InvalidProjectName(e) => e.into(),
            Error::ProjectNotFound(e) => e.into(),
            Error::Permission(e) => e.into(),
            Error::Service(e) => e.into(),
        }
    }
}

/// A wrapper for a guard that validates a user's API token *and*
/// scopes the request to a project they own.
///
/// It is guaranteed that [`ScopedUser::scope`] exists and is owned
/// by [`ScopedUser::name`].
#[derive(Clone)]
pub struct ScopedUser {
    pub claim: Claim,
    pub scope: ProjectName,
}

#[async_trait]
impl<S> FromRequestParts<S> for ScopedUser
where
    S: Send + Sync,
    RouterState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let claim = Claim::from_request_parts(parts, state)
            .await
            .map_err(Error::StatusCode)?;

        let RouterState { service, .. } = RouterState::from_ref(state);

        // Enables checking HEAD at /projects/{ulid}, used by res-rec to check permission for a proj id
        let scope = match Path::<Ulid>::from_request_parts(parts, state).await {
            Ok(Path(ulid)) => {
                let p = service.find_project_by_id(&ulid.to_string()).await?;
                ProjectName::new(&p.name).expect("valid project name")
            }
            Err(_) => {
                // Normal check for project name in path
                match Path::<ProjectName>::from_request_parts(parts, state).await {
                    Ok(Path(p)) => p,
                    Err(_) => Path::<(ProjectName, String)>::from_request_parts(parts, state)
                        .await
                        .map(|Path((p, _))| p)
                        .map_err(|_| InvalidProjectName)?,
                }
            }
        };

        if claim.is_admin()
            || claim.is_deployer()
            || service
                .permit_client
                .allowed(
                    &claim.sub,
                    &service.find_project_by_name(&scope).await?.id,
                    "develop", // TODO: make this configurable per endpoint?
                )
                .await?
        {
            Ok(Self { claim, scope })
        } else {
            Err(ProjectNotFound(scope.to_string()).into())
        }
    }
}
