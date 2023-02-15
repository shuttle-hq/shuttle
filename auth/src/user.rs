use std::{fmt::Formatter, str::FromStr};

use async_trait::async_trait;
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
        // TODO: generate a secret
        let secret = "my_secret".to_owned();

        query("INSERT INTO users (account_name, secret) VALUES (?1, ?2)")
            .bind(&name)
            .bind(&secret)
            .execute(&self.pool)
            .await?;

        Ok(User::new_with_defaults(name, secret))
    }

    // TODO: get from token?
    async fn get_user(&self, name: AccountName) -> Result<User, Error> {
        query("SELECT account_name, secret, super_user, account_tier FROM users WHERE account_name = ?1")
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
                    secret: row.try_get("secret").unwrap(),
                    permissions,
                }
            })
            .ok_or(Error::UserNotFound)
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize, Debug)]
pub struct User {
    pub name: AccountName,
    pub secret: String,
    pub permissions: Permissions,
}

impl User {
    #[allow(unused)]
    pub fn is_super_user(&self) -> bool {
        self.permissions.is_super_user()
    }

    pub fn new_with_defaults(name: AccountName, secret: String) -> Self {
        Self {
            name,
            secret,
            permissions: Permissions::default(),
        }
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
