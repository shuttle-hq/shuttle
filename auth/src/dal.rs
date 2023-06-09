use std::{fmt, path::Path, str::FromStr};

use crate::{
    user::{AccountName, AccountTier, User},
    Error,
};
use async_trait::async_trait;
use shuttle_common::ApiKey;
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    Row, SqlitePool,
};
use tracing::{error, info};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    Sqlx(#[from] sqlx::Error),
    UserNotFound,
}

// We are not using the `thiserror`'s `#[error]` syntax to prevent sensitive details from bubbling up to the users.
// Instead we are logging it as an error which we can inspect.
impl fmt::Display for DalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            DalError::Sqlx(error) => {
                error!(error = error.to_string(), "database request failed");

                "failed to interact with auth database"
            }
            DalError::UserNotFound => "user not found",
        };

        write!(f, "{msg}")
    }
}

#[async_trait]
pub trait Dal {
    /// Create a new account
    async fn create_user(&self, name: AccountName, tier: AccountTier) -> Result<User, DalError>;

    /// Get an account by [AccountName]
    async fn get_user(&self, name: AccountName) -> Result<User, DalError>;

    /// Get an account by [ApiKey]
    async fn get_user_by_key(&self, key: ApiKey) -> Result<User, DalError>;

    /// Reset an account's [ApiKey]
    async fn reset_key(&self, name: AccountName) -> Result<(), DalError>;
}

pub struct Sqlite {
    pool: SqlitePool,
}

impl Sqlite {
    /// This function creates all necessary tables and sets up a database connection pool.
    pub async fn new(path: &str) -> Self {
        if !Path::new(path).exists() {
            sqlx::Sqlite::create_database(path).await.unwrap();
        }

        info!(
            "state db: {}",
            std::fs::canonicalize(path).unwrap().to_string_lossy()
        );

        // We have found in the past that setting synchronous to anything other than the default (full) breaks the
        // broadcast channel in deployer. The broken symptoms are that the ws socket connections won't get any logs
        // from the broadcast channel and would then close. When users did deploys, this would make it seem like the
        // deploy is done (while it is still building for most of the time) and the status of the previous deployment
        // would be returned to the user.
        //
        // If you want to activate a faster synchronous mode, then also do proper testing to confirm this bug is no
        // longer present.
        let sqlite_options = SqliteConnectOptions::from_str(path)
            .unwrap()
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(sqlite_options).await.unwrap();

        Self::from_pool(pool).await
    }

    pub async fn insert_admin(&self, account_name: &str, key: Option<&str>) {
        let key = match key {
            Some(key) => ApiKey::parse(key).unwrap(),
            None => ApiKey::generate(),
        };

        sqlx::query("INSERT INTO users (account_name, key, account_tier) VALUES (?1, ?2, ?3)")
            .bind(account_name)
            .bind(&key)
            .bind(AccountTier::Admin)
            .execute(&self.pool)
            .await
            .expect("should be able to insert admin user, does it already exist?");

        println!(
            "`{}` created as super user with key: {}",
            account_name,
            key.as_ref()
        );
    }

    /// A utility for creating a migrating an in-memory database for testing.
    /// Currently only used for integration tests so the compiler thinks it is
    /// dead code.
    #[allow(dead_code)]
    pub async fn new_in_memory() -> Self {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Self {
        MIGRATIONS.run(&pool).await.unwrap();

        Self { pool }
    }
}

#[async_trait]
impl Dal for Sqlite {
    async fn create_user(&self, name: AccountName, tier: AccountTier) -> Result<User, DalError> {
        let key = ApiKey::generate();

        sqlx::query("INSERT INTO users (account_name, key, account_tier) VALUES (?1, ?2, ?3)")
            .bind(&name)
            .bind(&key)
            .bind(tier)
            .execute(&self.pool)
            .await?;

        Ok(User::new(name, key, tier))
    }

    async fn get_user(&self, name: AccountName) -> Result<User, DalError> {
        sqlx::query("SELECT account_name, key, account_tier FROM users WHERE account_name = ?1")
            .bind(&name)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| User {
                name,
                key: row.try_get("key").expect("a user should always have a key"),
                account_tier: row
                    .try_get("account_tier")
                    .expect("a user should always have an account tier"),
            })
            .ok_or(DalError::UserNotFound)
    }

    async fn get_user_by_key(&self, key: ApiKey) -> Result<User, DalError> {
        sqlx::query("SELECT account_name, key, account_tier FROM users WHERE key = ?1")
            .bind(&key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| User {
                name: row
                    .try_get("account_name")
                    .expect("a user should always have an account name"),
                key,
                account_tier: row
                    .try_get("account_tier")
                    .expect("a user should always have an account tier"),
            })
            .ok_or(DalError::UserNotFound)
    }

    async fn reset_key(&self, name: AccountName) -> Result<(), DalError> {
        let key = ApiKey::generate();

        let rows_affected = sqlx::query("UPDATE users SET key = ?1 WHERE account_name = ?2")
            .bind(&key)
            .bind(&name)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if rows_affected > 0 {
            Ok(())
        } else {
            Err(DalError::UserNotFound)
        }
    }
}
