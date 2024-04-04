use std::fmt::Debug;

use axum::extract::{FromRef, FromRequestParts, Path};
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};
use shuttle_backends::project_name::ProjectName;
use shuttle_backends::ClaimExt;
use shuttle_common::claims::Claim;
use shuttle_common::models::error::InvalidProjectName;
use shuttle_common::models::user::UserId;
use tracing::{error, trace, Span};

use crate::api::latest::RouterState;
use crate::{Error, ErrorKind};

/// A wrapper to enrich a token with user details
///
/// The `FromRequest` impl consumes the API claim and enriches it with project
/// details. Generally you want to use [`ScopedUser`] instead to ensure the request
/// is valid against the user's owned resources.
#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub claim: Claim,
    pub id: UserId,
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
    RouterState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let claim = parts.extensions.get::<Claim>().ok_or(ErrorKind::Internal)?;
        let user_id = claim.sub.clone();

        // Record current account name for tracing purposes
        Span::current().record("account.user_id", &user_id);

        let user = User {
            claim: claim.clone(),
            id: user_id,
        };

        trace!(?user, "got user");

        Ok(user)
    }
}

/// A wrapper for a guard that validates a user's API token *and*
/// scopes the request to a project they own.
///
/// It is guaranteed that [`ScopedUser::scope`] exists and is owned
/// by [`ScopedUser::name`].
#[derive(Clone)]
pub struct ScopedUser {
    pub user: User,
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
        let user = User::from_request_parts(parts, state).await?;

        let scope = match Path::<ProjectName>::from_request_parts(parts, state).await {
            Ok(Path(p)) => p,
            Err(_) => Path::<(ProjectName, String)>::from_request_parts(parts, state)
                .await
                .map(|Path((p, _))| p)
                .map_err(|_| Error::from(ErrorKind::InvalidProjectName(InvalidProjectName)))?,
        };

        let RouterState { service, .. } = RouterState::from_ref(state);

        let has_bypass = user.claim.is_admin() || user.claim.is_deployer();

        let allowed = has_bypass
            || {
                let projects: Vec<_> = service.iter_user_projects(&user.id).await?.collect();
                let internal_allowed = projects.contains(&scope);

                let permit_allowed = service
                    .permit_client
                    .allowed(
                        &user.id,
                        &service.find_project_by_name(&scope).await?.id,
                        "develop", // TODO?: make this configurable per endpoint?
                    )
                    .await
                    .map_err(|_| {
                        error!("failed to check Permit permission");
                        // Error::from_kind(ErrorKind::Internal)
                    })
                    .unwrap_or_default();

                if internal_allowed != permit_allowed {
                    error!(
                        "PERMIT: Permissions for user {} project {} did not match internal permissions. Internal: {}, Permit: {}",
                        user.id,
                        scope,
                        internal_allowed,
                        permit_allowed
                    );
                }

                internal_allowed
            };

        if allowed {
            Ok(Self { user, scope })
        } else {
            Err(Error::from(ErrorKind::ProjectNotFound(scope.to_string())))
        }
    }
}
