use std::{fmt::Formatter, str::FromStr};

use async_trait::async_trait;
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{query, Pool, Row, Sqlite};

use crate::error::Error;

#[async_trait]
pub(crate) trait UserManagement {
    async fn create_user(&self, name: AccountName) -> Result<User, Error>;
    async fn get_user(&self, name: AccountName) -> Result<User, Error>;
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

    // TODO: get from token?
    async fn get_user(&self, name: AccountName) -> Result<User, Error> {
        query(
            "SELECT account_name, key, super_user, account_tier FROM users WHERE account_name = ?1",
        )
        .bind(&name)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            let permissions = Permissions::builder()
                .super_user(row.try_get("super_user").unwrap())
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
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub name: AccountName,
    pub key: Key,
    pub permissions: Permissions,
}

impl User {
    #[allow(unused)]
    pub fn is_super_user(&self) -> bool {
        self.permissions.is_super_user()
    }

    pub fn new_with_defaults(name: AccountName, key: Key) -> Self {
        Self {
            name,
            key,
            permissions: Permissions::default(),
        }
    }
}

#[derive(Clone, Debug, sqlx::Type, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Key(String);

impl Key {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// #[async_trait]
// impl<S> FromRequestParts<S> for Key
// where
//     S: Send + Sync,
// {
//     type Rejection = Error;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let key = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
//             .await
//             .map_err(|_| Error::from(ErrorKind::KeyMissing))
//             .and_then(|TypedHeader(Authorization(bearer))| bearer.token().trim().parse())?;

//         trace!(%key, "got bearer key");

//         Ok(key)
//     }
// }

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

    #[allow(unused)]
    pub fn tier(&self) -> &AccountTier {
        &self.tier
    }

    #[allow(unused)]
    pub fn is_super_user(&self) -> bool {
        self.super_user
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
