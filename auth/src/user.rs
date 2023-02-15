use std::{fmt::Formatter, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{query_as, FromRow, Pool, Sqlite};

use crate::error::Error;

#[async_trait::async_trait]
pub(crate) trait UserManagement {
    async fn create_user(&self, name: UserName) -> Result<User, Error>;
    async fn get_user(&self, name: UserName) -> Result<User, Error>;
}

#[derive(Clone, FromRow, Serialize, Deserialize)]
pub(crate) struct User {
    pub name: UserName,
    pub secret: String,
}

#[derive(Clone)]
pub(crate) struct UserManager {
    pub(crate) pool: Pool<Sqlite>,
}

#[async_trait::async_trait]
impl UserManagement for UserManager {
    async fn create_user(&self, name: UserName) -> Result<User, Error> {
        // TODO: generate a secret
        let secret = "my_secret";

        let user = query_as("INSERT INTO users (user_name, key) VALUES (?1, ?2)")
            .bind(&name)
            .bind(secret)
            .fetch_one(&self.pool)
            .await?;

        Ok(user)
    }

    // TODO: get from token?
    async fn get_user(&self, name: UserName) -> Result<User, Error> {
        let user = query_as("SELECT user_name, secret FROM users WHERE user_name = ?1")
            .bind(&name)
            .fetch_one(&self.pool)
            .await?;

        Ok(user)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(transparent)]
pub(crate) struct UserName(String);

impl FromStr for UserName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Display for UserName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for UserName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|_err| todo!())
    }
}
