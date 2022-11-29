use std::fmt::{Debug, Formatter};
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Extension, FromRequestParts, Path, TypedHeader};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::request::Parts;
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use tracing::{trace, Span};

use crate::service::GatewayService;
use crate::{AccountName, Error, ErrorKind, ProjectName};

#[derive(Clone, Debug, sqlx::Type, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Key(String);

impl Key {
    pub fn as_str(&self) -> &str {
        &self.0
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
            .and_then(|TypedHeader(Authorization(bearer))| bearer.token().trim().parse())?;

        trace!(%key, "got bearer key");

        Ok(key)
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Key {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl Key {
    pub fn new_random() -> Self {
        Self(Alphanumeric.sample_string(&mut rand::thread_rng(), 16))
    }
}

/// A wrapper for a guard that verifies an API key is associated with a
/// valid user.
///
/// The `FromRequest` impl consumes the API key and verifies it is valid for the
/// a user. Generally you want to use [`ScopedUser`] instead to ensure the request
/// is valid against the user's owned resources.
#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub name: AccountName,
    pub key: Key,
    pub projects: Vec<ProjectName>,
    pub permissions: Permissions,
}

impl User {
    pub fn is_super_user(&self) -> bool {
        self.permissions.is_super_user()
    }

    pub fn new_with_defaults(name: AccountName, key: Key) -> Self {
        Self {
            name,
            key,
            projects: Vec::new(),
            permissions: Permissions::default(),
        }
    }

    pub async fn retrieve_from_account_name(
        svc: &GatewayService,
        name: AccountName,
    ) -> Result<User, Error> {
        let key = svc.key_from_account_name(&name).await?;
        let permissions = svc.get_permissions(&name).await?;
        let projects = svc.iter_user_projects(&name).await?.collect();
        Ok(User {
            name,
            key,
            projects,
            permissions,
        })
    }

    pub async fn retrieve_from_key(svc: &GatewayService, key: Key) -> Result<User, Error> {
        let name = svc.account_name_from_key(&key).await?;
        trace!(%name, "got account name from key");

        let permissions = svc.get_permissions(&name).await?;
        let projects = svc.iter_user_projects(&name).await?.collect();
        Ok(User {
            name,
            key,
            projects,
            permissions,
        })
    }
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize, Debug, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum AccountTier {
    Basic,
    Pro,
    Team,
}

#[derive(Default)]
pub struct PermissionsBuilder {
    tier: Option<AccountTier>,
    super_user: Option<bool>,
}

impl PermissionsBuilder {
    pub fn super_user(mut self, is_super_user: bool) -> Self {
        self.super_user = Some(is_super_user);
        self
    }

    pub fn tier(mut self, tier: AccountTier) -> Self {
        self.tier = Some(tier);
        self
    }

    pub fn build(self) -> Permissions {
        Permissions {
            tier: self.tier.unwrap_or(AccountTier::Basic),
            super_user: self.super_user.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct Permissions {
    pub tier: AccountTier,
    pub super_user: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            tier: AccountTier::Basic,
            super_user: false,
        }
    }
}

impl Permissions {
    pub fn builder() -> PermissionsBuilder {
        PermissionsBuilder::default()
    }

    pub fn tier(&self) -> &AccountTier {
        &self.tier
    }

    pub fn is_super_user(&self) -> bool {
        self.super_user
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key = Key::from_request_parts(parts, state).await?;
        let Extension(service) = Extension::<Arc<GatewayService>>::from_request_parts(parts, state)
            .await
            .unwrap();
        let user = User::retrieve_from_key(&service, key)
            .await
            // Absord any error into `Unauthorized`
            .map_err(|e| Error::source(ErrorKind::Unauthorized, e))?;

        // Record current account name for tracing purposes
        Span::current().record("account.name", &user.name.to_string());

        Ok(user)
    }
}

impl From<User> for shuttle_common::models::user::Response {
    fn from(user: User) -> Self {
        Self {
            name: user.name.to_string(),
            key: user.key.to_string(),
            projects: user
                .projects
                .into_iter()
                .map(|name| name.to_string())
                .collect(),
        }
    }
}

/// A wrapper for a guard that validates a user's API key *and*
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

        if user.is_super_user() || user.projects.contains(&scope) {
            Ok(Self { user, scope })
        } else {
            Err(Error::from(ErrorKind::ProjectNotFound))
        }
    }
}

pub struct Admin {
    pub user: User,
}

#[async_trait]
impl<S> FromRequestParts<S> for Admin
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = User::from_request_parts(parts, state).await?;
        if user.is_super_user() {
            Ok(Self { user })
        } else {
            Err(Error::from(ErrorKind::Forbidden))
        }
    }
}
