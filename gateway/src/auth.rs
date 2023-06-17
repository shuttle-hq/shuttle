use std::fmt::Debug;
use std::str::FromStr;

use axum::extract::{FromRef, FromRequestParts, Path};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::request::Parts;
use axum::TypedHeader;
use serde::{Deserialize, Serialize};
use shuttle_common::claims::{Claim, Scope};
use shuttle_common::ApiKey;
use tonic::metadata::{MetadataMap, MetadataValue};
use tracing::{debug, error, trace, Span};

use crate::api::latest::RouterState;
use crate::{AccountName, Error, ErrorKind, ProjectName};

/// A wrapper to enrich a token with user details
///
/// The `FromRequest` impl consumes the API claim and enriches it with project
/// details. Generally you want to use [`ScopedUser`] instead to ensure the request
/// is valid against the user's owned resources.
#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub projects: Vec<ProjectName>,
    pub claim: Claim,
    pub name: AccountName,
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
    RouterState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let claim = parts.extensions.get::<Claim>().ok_or(ErrorKind::Internal)?;
        let name = AccountName::from_str(&claim.sub)
            .map_err(|err| Error::source(ErrorKind::Internal, err))?;

        // Record current account name for tracing purposes
        Span::current().record("account.name", &name.to_string());

        let RouterState { service, .. } = RouterState::from_ref(state);

        let user = User {
            claim: claim.clone(),
            projects: service.iter_user_projects(&name).await?.collect(),
            name,
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
                .unwrap(),
        };

        if user.projects.contains(&scope) || user.claim.scopes.contains(&Scope::Admin) {
            Ok(Self { user, scope })
        } else {
            Err(Error::from(ErrorKind::ProjectNotFound))
        }
    }
}

/// A wrapper around [ApiKey] so we can implement [FromRequestParts] for it.
pub struct Key(ApiKey);

impl From<Key> for ApiKey {
    fn from(key: Key) -> Self {
        key.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Key
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::from(ErrorKind::KeyMissing))
            .and_then(|TypedHeader(Authorization(bearer))| {
                let bearer = bearer.token().trim();
                ApiKey::parse(bearer).map_err(|error| {
                    debug!(error = ?error, "received a malformed api-key");
                    Self::Rejection::from(ErrorKind::KeyMalformed)
                })
            })?;

        trace!("got bearer key");

        Ok(Key(key))
    }
}

/// Utility function to extract a "set-cookie" cookie from a tonic [MetadataMap], that also takes
/// a request name for debug logging.
pub(crate) fn extract_metadata_cookie<'a>(
    metadata: &'a MetadataMap,
    request_name: &str,
) -> Result<&'a str, Error> {
    metadata.get("set-cookie")
    .ok_or({
        debug!("failed to get set-cookie cookie from {request_name} request");
        Error::from_kind(ErrorKind::Internal)
    })?
    .to_str()
    .map_err(|error| {
        debug!(error = ?error, "set-cookie received from {request_name} request has invalid metadata characters");
        Error::from_kind(ErrorKind::Internal)
    })
}

/// Utility function that inserts a bearer token in a tonic request [MetadataMap], useful for
/// endpoints that expect a bearer token in the following format:
///
/// `authorization Bearer <api-key>`
pub(crate) fn insert_metadata_bearer_token(
    metadata: &mut MetadataMap,
    key: Key,
) -> Result<(), Error> {
    let bearer: MetadataValue<_> = format!("Bearer {}", shuttle_common::ApiKey::from(key).as_ref())
        .parse()
        .map_err(|error| {
            // This should be impossible since an ApiKey can only contain valid valid characters.
            error!(error = ?error, "api-key contains invalid metadata characters");

            Error::from_kind(ErrorKind::Internal)
        })?;

    metadata.insert("authorization", bearer);

    Ok(())
}
