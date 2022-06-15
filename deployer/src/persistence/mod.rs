use crate::deployment::DeploymentInfo;

use std::path::Path;

use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{Sqlite, SqlitePool};

use anyhow::anyhow;

const DB_PATH: &str = "deployer.sqlite";

#[derive(Clone)]
pub struct Persistence {
    pool: SqlitePool,
}

impl Persistence {
    /// Creates a persistent storage solution (i.e., SQL database). This
    /// function creates all necessary tables and sets up a database connection
    /// pool - new connections should be made by cloning [`Persistence`] rather
    /// than repeatedly calling [`Persistence::new`].
    pub async fn new() -> Self {
        if !Path::new(DB_PATH).exists() {
            Sqlite::create_database(DB_PATH).await.unwrap();
        }

        let pool = SqlitePool::connect(DB_PATH).await.unwrap();

        // TODO: Consider indices/keys.

        sqlx::query("
            CREATE TABLE IF NOT EXISTS deploying (
                name TEXT UNIQUE, -- Name of the service being deployed.
                state INTEGER     -- Enum indicating the current state of the deployment.
            );

            CREATE TABLE IF NOT EXISTS active_deployments (
                name TEXT UNIQUE -- Name of the active deployment.
            );

            CREATE TABLE IF NOT EXISTS logs (
                text TEXT,        -- Log line(s).
                name TEXT,        -- The service that this log line pertains to.
                state INTEGER,    -- The state of the deployment at the time at which the log text was produced.
                timestamp INTEGER -- Unix eopch timestamp.
            );
        ").execute(&pool).await.unwrap();

        Persistence { pool }
    }

    pub async fn deployment(&self, info: DeploymentInfo) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO deploying (name, state) VALUES (?, ?)")
            .bind(info.name)
            .bind(info.state)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("Failed to update/insert deployment data: {}", e))
    }
}
