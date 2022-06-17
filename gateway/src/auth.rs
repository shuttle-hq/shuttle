use serde::{Deserialize, Serialize};
use serde_json::json;

use rand::Rng;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{FromRequest, Extension, RequestParts, TypedHeader, Path};
use axum::headers::{authorization::Basic, Authorization};
use axum::response::{IntoResponse, Response};
use std::str::FromStr;
use axum::http::StatusCode;
use axum::Json;

use crate::service::GatewayService;
use crate::{Error, ErrorKind, ProjectName, AccountName};

#[derive(Clone, Debug, sqlx::Type, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Key(pub String);

#[async_trait]
impl<B> FromRequest<B> for Key
where
    B: Send
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        TypedHeader::<Authorization<Basic>>::from_request(req)
            .await
            .map_err(|_| Error::from(ErrorKind::KeyMissing))
            .and_then(|TypedHeader(Authorization(basic))| basic.password().parse())
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
        Self(
            rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 16)
        )
    }
}

/// A wrapper for a guard that verifies an API key is associated with a
/// valid user.
///
/// The `FromRequest` impl consumes the API key and verifies it is valid for the
/// a user. Generally you want to use [`ScopedUser`] instead to ensure the request
/// is valid against the user's owned resources.
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct User {
    pub name: AccountName,
    pub key: Key,
    pub projects: Vec<ProjectName>
}

#[async_trait]
impl<B> FromRequest<B> for User
where
    B: Send
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let key = Key::from_request(req).await?;
        let Extension(service) = Extension::<Arc<GatewayService>>::from_request(req)
            .await
            .unwrap();
        let user = service.user_from_key(key)
            .await
            // Absord any error into `Unauthorized`
            .map_err(|_| {
                Error::kind(ErrorKind::Unauthorized)
            })?;
        Ok(user)
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
impl<B> FromRequest<B> for ScopedUser
where
    B: Send
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let user = User::from_request(req).await?;
        let scope = match Path::<ProjectName>::from_request(req).await {
            Ok(Path(p)) => p,
            Err(_) => Path::<(ProjectName, String)>::from_request(req)
                .await
                .map(|Path((p, _))| p)
                .unwrap()
        };
        if user.projects.contains(&scope) {
            Ok(Self { user, scope })
        } else {
            Err(Error::from(ErrorKind::Forbidden))
        }
    }
}

pub struct Admin {
    pub user: User,
}

#[async_trait]
impl<B> FromRequest<B> for Admin
where
    B: Send
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let user = User::from_request(req).await?;
        let service = Extension::<Arc<GatewayService>>::from_request(req).await.unwrap();
        if service.is_super_user(&user.name).await? {
            Ok(Self { user })
        } else {
            Err(Error::from(ErrorKind::Forbidden))
        }
    }
}
