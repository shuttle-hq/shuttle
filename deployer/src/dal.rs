use std::fmt;
use std::path::Path;
use std::str::FromStr;

use axum::async_trait;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{migrate::Migrator, SqlitePool};
use thiserror::Error;
use tracing::{error, info};
use ulid::Ulid;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    Sqlx(#[from] sqlx::Error),
    ProjectNotFound,
}

// We are not using the `thiserror`'s `#[error]` syntax to prevent sensitive details from bubbling up to the users.
// Instead we are logging it as an error which we can inspect.
impl fmt::Display for DalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            DalError::Sqlx(error) => {
                error!(error = error.to_string(), "database request failed");

                "failed to interact with recorder"
            }
            DalError::ProjectNotFound => "the queried project couldn't be found",
        };

        write!(f, "{msg}")
    }
}

#[async_trait]
pub trait Dal {
    /// Fetch project state if project exists.
    async fn service_state(&self, service_id: &Ulid) -> Result<(), DalError>;
    /// Update the project information.
    async fn update_service_state(&self, service_id: &Ulid, project: ()) -> Result<(), DalError>;
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
    async fn service_state(&self, service_id: &Ulid) -> Result<(), DalError> {
        Ok(())
    }

    // async fn account(&self, service_id: &Ulid) -> Result<AccountName, DalError> {
    //     Ok(
    //         query("SELECT account_name FROM projects WHERE service_id = ?1")
    //             .bind(service_id.to_string())
    //             .fetch_optional(&self.pool)
    //             .await?
    //             .ok_or(DalError::ProjectNotFound)?
    //             .get("account_name"),
    //     )
    // }

    async fn update_service_state(&self, service_id: &Ulid, project: ()) -> Result<(), DalError> {
        // let query = match project {
        //     ServiceState::Creating(state) => {
        //         query("UPDATE projects SET initial_key = ?1, state = ?2 WHERE service_id = ?3")
        //             .bind(state.initial_key())
        //             .bind(SqlxJson(project))
        //             .bind(service_id.to_string())
        //     }
        //     _ => query("UPDATE projects SET state = ?1 WHERE service_id = ?2")
        //         .bind(SqlxJson(project))
        //         .bind(service_id.to_string()),
        // };

        // query
        //     .execute(&self.pool)
        //     .await
        //     .map_err(|err| DalError::Sqlx(err))?;

        Ok(())
    }
}
