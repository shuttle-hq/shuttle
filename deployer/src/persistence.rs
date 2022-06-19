use crate::deployment::{DeploymentInfo, DeploymentState};
use crate::error::Result;

use std::path::Path;

use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{Sqlite, SqlitePool};

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
        Self::from_pool(pool).await
    }

    #[allow(dead_code)]
    async fn new_in_memory() -> Self {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Self {
        sqlx::query("
            CREATE TABLE IF NOT EXISTS deployments (
                name TEXT PRIMARY KEY, -- Name of the service being deployed.
                state INTEGER          -- Enum indicating the current state of the deployment.
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

    pub async fn update_deployment(&self, info: impl Into<DeploymentInfo>) -> Result<()> {
        let info = info.into();

        // TODO: Handle moving to 'active_deployments' table for DeploymentState::Running.

        sqlx::query("INSERT OR REPLACE INTO deployments (name, state) VALUES (?, ?)")
            .bind(info.name)
            .bind(info.state)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn get_deployment(&self, name: &str) -> Result<Option<DeploymentInfo>> {
        sqlx::query_as("SELECT * FROM deployments WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn delete_deployment(&self, name: &str) -> Result<Option<DeploymentInfo>> {
        let info = self.get_deployment(name).await?;

        let _ = sqlx::query("DELETE FROM deployments WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(info)
    }

    pub async fn get_all_deployments(&self) -> Result<Vec<DeploymentInfo>> {
        sqlx::query_as("SELECT * FROM deployments")
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_all_runnable_deployments(&self) -> Result<Vec<DeploymentInfo>> {
        sqlx::query_as("SELECT * FROM deployments WHERE state = ? OR state = ?")
            .bind(DeploymentState::Built)
            .bind(DeploymentState::Running)
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::Built;

    #[tokio::test]
    async fn deployment_updates() {
        let p = Persistence::new_in_memory().await;

        let mut info = DeploymentInfo {
            name: "abc".to_string(),
            state: DeploymentState::Queued,
        };

        p.update_deployment(info.clone()).await.unwrap();
        assert_eq!(p.get_deployment("abc").await.unwrap().unwrap(), info);

        p.update_deployment(&Built {
            name: "abc".to_string(),
            state: DeploymentState::Built,
        })
        .await
        .unwrap();
        info.state = DeploymentState::Built;
        assert_eq!(p.get_deployment("abc").await.unwrap().unwrap(), info);
    }

    #[tokio::test]
    async fn fetching_runnable_deployments() {
        let p = Persistence::new_in_memory().await;

        for info in [
            DeploymentInfo {
                name: "abc".to_string(),
                state: DeploymentState::Queued,
            },
            DeploymentInfo {
                name: "foo".to_string(),
                state: DeploymentState::Built,
            },
            DeploymentInfo {
                name: "bar".to_string(),
                state: DeploymentState::Running,
            },
            DeploymentInfo {
                name: "def".to_string(),
                state: DeploymentState::Building,
            },
        ] {
            p.update_deployment(info).await.unwrap();
        }

        let runnable = p.get_all_runnable_deployments().await.unwrap();
        assert!(!runnable.iter().any(|x| x.name == "abc"));
        assert!(runnable.iter().any(|x| x.name == "foo"));
        assert!(runnable.iter().any(|x| x.name == "bar"));
        assert!(!runnable.iter().any(|x| x.name == "def"));
    }

    #[tokio::test]
    async fn deployment_deletion() {
        let p = Persistence::new_in_memory().await;

        p.update_deployment(DeploymentInfo {
            name: "x".to_string(),
            state: DeploymentState::Running,
        })
        .await
        .unwrap();
        assert!(p.get_deployment("x").await.unwrap().is_some());
        p.delete_deployment("x").await.unwrap();
        assert!(p.get_deployment("x").await.unwrap().is_none());
    }
}
