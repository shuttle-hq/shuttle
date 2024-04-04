use axum::extract::{FromRef, FromRequestParts, Path};
use axum::http::request::Parts;
use shuttle_backends::project_name::ProjectName;
use shuttle_backends::ClaimExt;
use shuttle_common::claims::Claim;
use shuttle_common::models::error::InvalidProjectName;
use tracing::error;

use crate::api::latest::RouterState;
use crate::{Error, ErrorKind};

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
            .map_err(|_| ErrorKind::Unauthorized)?;

        let scope = match Path::<ProjectName>::from_request_parts(parts, state).await {
            Ok(Path(p)) => p,
            Err(_) => Path::<(ProjectName, String)>::from_request_parts(parts, state)
                .await
                .map(|Path((p, _))| p)
                .map_err(|_| Error::from(ErrorKind::InvalidProjectName(InvalidProjectName)))?,
        };

        let RouterState { service, .. } = RouterState::from_ref(state);

        let allowed = claim.is_admin()
            || claim.is_deployer()
            || service
                .permit_client
                .allowed(
                    &claim.sub,
                    &service.find_project_by_name(&scope).await?.id,
                    "develop", // TODO: make this configurable per endpoint?
                )
                .await
                .map_err(|_| {
                    error!("failed to check Permit permission");
                    Error::from_kind(ErrorKind::Internal)
                })?;

        if allowed {
            Ok(Self { claim, scope })
        } else {
            Err(Error::from(ErrorKind::ProjectNotFound(scope.to_string())))
        }
    }
}
