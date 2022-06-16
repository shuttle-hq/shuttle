use crate::deployment::{DeploymentInfo, DeploymentState};

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
        // TODO: Is having a separate table for active deployments necessary?

        sqlx::query("
            CREATE TABLE IF NOT EXISTS deployments (
                name TEXT UNIQUE, -- Name of the service being deployed.
                state INTEGER     -- Enum indicating the current state of the deployment.
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

    pub async fn update_deployment(&self, info: impl Into<DeploymentInfo>) -> anyhow::Result<()> {
        let info = info.into();

        // TODO: Handle moving to 'active_deployments' table for DeploymentState::Running.

        sqlx::query("INSERT OR REPLACE INTO deployments (name, state) VALUES (?, ?)")
            .bind(info.name)
            .bind(info.state)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("Failed to update/insert deployment data: {e}"))
    }

    pub async fn get_deployment(&self, name: &str) -> anyhow::Result<DeploymentInfo> {
        sqlx::query_as("SELECT * FROM deployments WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Could not get deployment data: {e}"))
    }

    pub async fn delete_deployment(&self, name: &str) -> anyhow::Result<DeploymentInfo> {
        let info = self
            .get_deployment(name)
            .await
            .map_err(|e| anyhow!("Failed to remove deployment data: {e}"))?;

        let _ = sqlx::query("DELETE FROM deployments WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(info)
    }

    pub async fn get_all_deployments(&self) -> anyhow::Result<Vec<DeploymentInfo>> {
        sqlx::query_as("SELECT * FROM deployments")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to get all deployment data: {e}"))
    }

    pub async fn get_all_runnable_deployments(&self) -> anyhow::Result<Vec<DeploymentInfo>> {
        sqlx::query_as("SELECT * FROM deployments WHERE state = ? OR state = ?")
            .bind(DeploymentState::Built)
            .bind(DeploymentState::Running)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to get all deployment data: {e}"))
    }
}
