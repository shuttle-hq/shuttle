use std::fmt;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

use axum::async_trait;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::types::Json as SqlxJson;
use sqlx::{migrate::Migrator, Row, SqlitePool};
use sqlx::{query, FromRow};
use thiserror::Error;
use tracing::{error, info};
use ulid::Ulid;

use crate::deployment::{Deployment, DeploymentRunnable, DeploymentState};
use crate::project::service::ServiceState;

use super::{Log, Service, State};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    Sqlx(#[from] sqlx::Error),
    ServiceNotFound,
    Decode(ulid::DecodeError),
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
            DalError::ServiceNotFound => "service not found",
            DalError::Decode(_) => "service id couldn't be decoded to Ulid",
        };

        write!(f, "{msg}")
    }
}

#[async_trait]
pub trait Dal: Send + Clone {
    // Have the dal connected to the log service.
    async fn insert_log(&self, log: Log) -> Result<(), DalError>;

    // Get a service by id
    async fn service(&self, id: &Ulid) -> Result<Service, DalError>;

    // Insert a service if absent
    async fn insert_service_if_absent(&self, service: Service) -> Result<bool, DalError>;

    // Insert a new deployment
    async fn insert_deployment(&self, deployment: Deployment) -> Result<(), DalError>;

    // Update all deployment invalid states inside persistence to `Stopped`.
    async fn update_invalid_states_to_stopped(&self) -> Result<(), DalError>;

    // Get runnning or runnable deployments
    async fn running_deployments(&self) -> Result<Vec<DeploymentRunnable>, DalError>;

    // Get the service state
    async fn service_state(&self, service_id: &Ulid) -> Result<Option<ServiceState>, DalError>;

    // Update a deployment state
    async fn update_deployment_state(&self, state: DeploymentState) -> Result<(), DalError>;

    // Update the project information.
    async fn update_service_state(
        &self,
        service_id: Ulid,
        state: ServiceState,
    ) -> Result<(), DalError>;

    // Get service running deployments
    async fn service_running_deployments(&self, service_id: &Ulid) -> Result<Vec<Ulid>, DalError>;

    // Get services
    async fn services(&self) -> Result<Vec<Service>, DalError>;

    // Set the deployment address
    async fn set_address(&self, id: &Ulid, address: &SocketAddr) -> Result<(), DalError>;

    // Set whether is a shuttle-next runtime
    async fn set_is_next(&self, id: &Ulid, is_next: bool) -> Result<(), DalError>;
}

#[derive(Clone)]
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
    async fn insert_log(&self, log: Log) -> Result<(), DalError> {
        Ok(())
    }

    async fn service_state(&self, service_id: &Ulid) -> Result<Option<ServiceState>, DalError> {
        query(
            r#"
            SELECT state
            FROM services
            WHERE (id = ?1)
            "#,
        )
        .bind(&service_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DalError::from)
        .map(|row| row.map(|inner| inner.get::<SqlxJson<ServiceState>, _>("state").0))
    }

    async fn service(&self, id: &Ulid) -> Result<Service, DalError> {
        let row = sqlx::query("SELECT * FROM services WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(DalError::from)?
            .ok_or(DalError::ServiceNotFound)?;
        Service::from_row(&row).map_err(DalError::Sqlx)
    }

    async fn insert_service_if_absent(&self, service: Service) -> Result<bool, DalError> {
        let Service {
            id,
            name,
            state_variant,
            state,
        } = service;

        if self.service(&id).await.is_ok() {
            return Ok(false);
        }

        sqlx::query("INSERT INTO services (id, name, state_variant, state) VALUES (?, ?, ?, ?)")
            .bind(id.to_string())
            .bind(name)
            .bind(state_variant)
            .bind(SqlxJson(state))
            .execute(&self.pool)
            .await?;
        Ok(true)
    }

    async fn insert_deployment(&self, deployment: Deployment) -> Result<(), DalError> {
        sqlx::query(
            "INSERT INTO deployments (id, service_id, state, last_update, address, is_next, git_commit_hash, git_commit_message, git_branch, git_dirty) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(deployment.id.to_string())
        .bind(deployment.service_id.to_string())
        .bind(deployment.state)
        .bind(deployment.last_update)
        .bind(deployment.address.map(|socket| socket.to_string()))
        .bind(deployment.is_next)
        .bind(deployment.git_commit_hash)
        .bind(deployment.git_commit_message)
        .bind(deployment.git_branch)
        .bind(deployment.git_dirty)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DalError::from)
    }

    async fn update_invalid_states_to_stopped(&self) -> Result<(), DalError> {
        sqlx::query("UPDATE deployments SET state = ? WHERE state IN(?, ?, ?, ?)")
            .bind(State::Stopped)
            .bind(State::Built)
            .bind(State::Building)
            .bind(State::Loading)
            .execute(&self.pool)
            .await
            .map_err(DalError::from);
        Ok(())
    }

    async fn running_deployments(&self) -> Result<Vec<DeploymentRunnable>, DalError> {
        sqlx::query_as(
            r#"SELECT d.id as id, service_id, s.name AS service_name, d.is_next as is_next
                    FROM deployments AS d
                    JOIN services AS s ON s.id = d.service_id
                    WHERE d.state = ?
                    ORDER BY last_update"#,
        )
        .bind(State::Running)
        .fetch_all(&self.pool)
        .await
        .map_err(DalError::from)
    }

    async fn update_deployment_state(&self, state: DeploymentState) -> Result<(), DalError> {
        sqlx::query("UPDATE deployments SET state = ?, last_update = ? WHERE id = ?")
            .bind(state.state)
            .bind(state.last_update)
            .bind(state.id.to_string())
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| DalError::from(e))
    }

    async fn update_service_state(
        &self,
        service_id: Ulid,
        state: ServiceState,
    ) -> Result<(), DalError> {
        let query = query("UPDATE services SET state = ?1 WHERE service_id = ?2")
            .bind(SqlxJson(state))
            .bind(service_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|err| DalError::Sqlx(err))?;

        Ok(())
    }

    async fn service_running_deployments(&self, service_id: &Ulid) -> Result<Vec<Ulid>, DalError> {
        let ids = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM deployments WHERE service_id = ? AND state = ?",
        )
        .bind(service_id.to_string())
        .bind(State::Running)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| DalError::Sqlx(err))?
        .into_iter()
        .map(|deployment| deployment.id)
        .collect();

        Ok(ids)
    }

    async fn set_address(&self, service_id: &Ulid, address: &SocketAddr) -> Result<(), DalError> {
        sqlx::query("UPDATE deployments SET address = ? WHERE id = ?")
            .bind(address.to_string())
            .bind(service_id.to_string())
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| DalError::Sqlx(err))
    }

    async fn set_is_next(&self, service_id: &Ulid, is_next: bool) -> Result<(), DalError> {
        sqlx::query("UPDATE deployments SET is_next = ? WHERE id = ?")
            .bind(is_next)
            .bind(service_id.to_string())
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| DalError::Sqlx(err))
    }

    // Get services
    async fn services(&self) -> Result<Vec<Service>, DalError> {
        let services: Result<Vec<Service>, DalError> = query("SELECT & FROM services")
            .fetch_all(&self.pool)
            .await
            .map_err(DalError::Sqlx)?
            .iter()
            .map(|row| Service::from_row(row).map_err(DalError::Sqlx))
            .collect();
        services
    }
}
