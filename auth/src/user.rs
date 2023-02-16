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
use sqlx::{query, Pool, Row, Sqlite};
use tracing::{trace, Span};

use crate::{api::RouterState, error::Error};

#[async_trait]
pub(crate) trait UserManagement {
    async fn create_user(&self, name: AccountName) -> Result<User, Error>;
    async fn get_user(&self, name: AccountName) -> Result<User, Error>;
    async fn get_user_by_key(&self, key: Key) -> Result<User, Error>;
}

#[derive(Clone)]
pub(crate) struct UserManager {
    pub(crate) pool: Pool<Sqlite>,
}

#[async_trait]
impl UserManagement for UserManager {
    async fn create_user(&self, name: AccountName) -> Result<User, Error> {
        let key = Key::new_random();

        query("INSERT INTO users (account_name, key) VALUES (?1, ?2)")
            .bind(&name)
            .bind(&key)
            .execute(&self.pool)
            .await?;

        Ok(User::new_with_defaults(name, key))
    }

    async fn get_user(&self, name: AccountName) -> Result<User, Error> {
        query("SELECT account_name, key, account_tier FROM users WHERE account_name = ?1")
            .bind(&name)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| {
                let permissions = Permissions::builder()
                    .tier(row.try_get("account_tier").unwrap())
                    .build();

                User {
                    name,
                    key: row.try_get("key").unwrap(),
                    permissions,
                }
            })
            .ok_or(Error::UserNotFound)
    }

    async fn get_user_by_key(&self, key: Key) -> Result<User, Error> {
        query("SELECT account_name, key, account_tier FROM users WHERE key = ?1")
            .bind(&key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| {
                let permissions = Permissions::builder()
                    .tier(row.try_get("account_tier").unwrap())
                    .build();

                User {
                    name: row.try_get("account_name").unwrap(),
                    key,
                    permissions,
                }
            })
            .ok_or(Error::UserNotFound)
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub name: AccountName,
    pub key: Key,
    pub permissions: Permissions,
}

impl User {
    #[allow(unused)]
    pub fn is_admin(&self) -> bool {
        self.permissions.tier() == &AccountTier::Admin
    }

    pub fn new_with_defaults(name: AccountName, key: Key) -> Self {
        Self {
            name,
            key,
            permissions: Permissions::default(),
        }
    }

    async fn retrieve_from_key(user_manager: &UserManager, key: Key) -> Result<User, Error> {
        let user = user_manager.get_user_by_key(key).await?;

        trace!(%user.name, "got account from key");

        Ok(user)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
    RouterState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key = Key::from_request_parts(parts, state).await?;

        let RouterState { user_manager } = RouterState::from_ref(state);

        let user = User::retrieve_from_key(&user_manager, key)
            .await
            // Absord any error into `Unauthorized`
            .map_err(|_| Error::Unauthorized)?;

        // Record current account name for tracing purposes
        Span::current().record("account.name", &user.name.to_string());

        Ok(user)
    }
}

#[derive(Clone, Debug, sqlx::Type, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Key(String);

// impl Key {
//     pub fn as_str(&self) -> &str {
//         &self.0
//     }
// }

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

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize, Debug, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum AccountTier {
    Basic,
    Pro,
    Team,
    Admin,
}

#[derive(Default)]
pub struct PermissionsBuilder {
    tier: Option<AccountTier>,
}

impl PermissionsBuilder {
    pub fn tier(mut self, tier: AccountTier) -> Self {
        self.tier = Some(tier);
        self
    }

    pub fn build(self) -> Permissions {
        Permissions {
            tier: self.tier.unwrap_or(AccountTier::Basic),
        }
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct Permissions {
    pub tier: AccountTier,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            tier: AccountTier::Basic,
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
            .map_err(|_err| todo!())
    }
}

pub struct Admin {
    pub user: User,
}

#[async_trait]
impl<S> FromRequestParts<S> for Admin
where
    S: Send + Sync,
    RouterState: FromRef<S>,
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
