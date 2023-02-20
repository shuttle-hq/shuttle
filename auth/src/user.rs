use std::{fmt::Formatter, str::FromStr};

use async_trait::async_trait;
use axum::{
    extract::{FromRef, FromRequestParts},
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    TypedHeader,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Deserializer, Serialize};
use shuttle_common::backends::auth::Scope;
use sqlx::{query, Row, SqlitePool};
use tracing::{trace, Span};

use crate::{api::UserManagerState, error::Error};

#[async_trait]
pub trait UserManagement: Send + Sync {
    async fn create_user(&self, name: AccountName, tier: AccountTier) -> Result<User, Error>;
    async fn get_user(&self, name: AccountName) -> Result<User, Error>;
    async fn get_user_by_key(&self, key: Key) -> Result<User, Error>;
}

#[derive(Clone)]
pub struct UserManager {
    pub pool: SqlitePool,
}

#[async_trait]
impl UserManagement for UserManager {
    async fn create_user(&self, name: AccountName, tier: AccountTier) -> Result<User, Error> {
        let key = Key::new_random();

        query("INSERT INTO users (account_name, key, account_tier) VALUES (?1, ?2, ?3)")
            .bind(&name)
            .bind(&key)
            .bind(tier)
            .execute(&self.pool)
            .await?;

        Ok(User::new(name, key, tier))
    }

    async fn get_user(&self, name: AccountName) -> Result<User, Error> {
        query("SELECT account_name, key, account_tier FROM users WHERE account_name = ?1")
            .bind(&name)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| User {
                name,
                key: row.try_get("key").unwrap(),
                account_tier: row.try_get("account_tier").unwrap(),
            })
            .ok_or(Error::UserNotFound)
    }

    async fn get_user_by_key(&self, key: Key) -> Result<User, Error> {
        query("SELECT account_name, key, account_tier FROM users WHERE key = ?1")
            .bind(&key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| User {
                name: row.try_get("account_name").unwrap(),
                key,
                account_tier: row.try_get("account_tier").unwrap(),
            })
            .ok_or(Error::UserNotFound)
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub name: AccountName,
    pub key: Key,
    pub account_tier: AccountTier,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.account_tier == AccountTier::Admin
    }

    pub fn new(name: AccountName, key: Key, account_tier: AccountTier) -> Self {
        Self {
            name,
            key,
            account_tier,
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
    UserManagerState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key = Key::from_request_parts(parts, state).await?;

        let user_manager: UserManagerState = UserManagerState::from_ref(state);

        let user = user_manager
            .get_user_by_key(key)
            .await
            // Absord any error into `Unauthorized`
            .map_err(|_| Error::Unauthorized)?;

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
            account_tier: user.account_tier.to_string(),
        }
    }
}

#[derive(Clone, Debug, sqlx::Type, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Key(String);

#[async_trait]
impl<S> FromRequestParts<S> for Key
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::KeyMissing)
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

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize, Debug, sqlx::Type, strum::Display)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum AccountTier {
    Basic,
    Pro,
    Team,
    Admin,
}

impl Default for AccountTier {
    fn default() -> Self {
        AccountTier::Basic
    }
}

impl From<AccountTier> for Vec<Scope> {
    fn from(tier: AccountTier) -> Self {
        let mut base = vec![
            Scope::Deployment,
            Scope::DeploymentPush,
            Scope::Logs,
            Scope::Project,
            Scope::ProjectCreate,
            Scope::Resources,
            Scope::ResourcesWrite,
            Scope::Secret,
            Scope::SecretWrite,
        ];

        if tier == AccountTier::Admin {
            base.append(&mut vec![
                Scope::User,
                Scope::UserCreate,
                Scope::AcmeCreate,
                Scope::CustomDomainCreate,
                Scope::Admin,
            ]);
        }

        base
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(transparent)]
pub struct AccountName(String);

impl FromStr for AccountName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Display for AccountName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for AccountName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

pub struct Admin {
    pub user: User,
}

#[async_trait]
impl<S> FromRequestParts<S> for Admin
where
    S: Send + Sync,
    UserManagerState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = User::from_request_parts(parts, state).await?;

        if user.is_admin() {
            Ok(Self { user })
        } else {
            Err(Error::Forbidden)
        }
    }
}
