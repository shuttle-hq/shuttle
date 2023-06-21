use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use axum::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow};
use sqlx::types::Json as SqlxJson;
use sqlx::{migrate::Migrator, Row, SqlitePool};
use sqlx::{query, FromRow};
use thiserror::Error;
use tracing::{error, info};
use ulid::Ulid;

use crate::project::docker::ContainerInspectResponseExt;
use crate::project::service::state::f_running::ServiceRunning;
use crate::project::service::state::StateVariant;
use crate::project::service::ServiceState;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    Sqlx(#[from] sqlx::Error),
    ServiceNotFound,
    DeploymentNotFound,
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
            DalError::DeploymentNotFound => "deployment not found",
            DalError::Decode(_) => "service id couldn't be decoded to Ulid",
        };

        write!(f, "{msg}")
    }
}

#[async_trait]
pub trait Dal: Send + Clone {
    // Get a service by id
    async fn service(&self, id: &Ulid) -> Result<Service, DalError>;

    // Get a deployment by id
    async fn deployment(&self, id: &Ulid) -> Result<Deployment, DalError>;

    // Insert a service if absent
    async fn insert_service_if_absent(&self, service: Service) -> Result<bool, DalError>;

    // Insert a new deployment
    async fn insert_deployment(&self, deployment: Deployment) -> Result<(), DalError>;

    // Get runnning or runnable deployments
    async fn running_deployments(&self) -> Result<Vec<RunningDeployment>, DalError>;

    // Get the service state
    async fn service_state(&self, service_id: &Ulid) -> Result<Option<ServiceState>, DalError>;

    // Update the project information.
    async fn update_service_state(
        &self,
        service_id: Ulid,
        state: ServiceState,
    ) -> Result<(), DalError>;

    // Get services
    async fn services(&self) -> Result<Vec<Service>, DalError>;
}

#[derive(Clone)]
pub struct Sqlite {
    pool: SqlitePool,
}

impl Sqlite {
    /// This function creates all necessary tables and sets up a database connection pool.
    pub async fn new(path: &PathBuf) -> Self {
        let path_as_str = path
            .to_str()
            .expect("to have a valid path for the sqlite db creation");
        if !path.as_path().exists() {
            sqlx::Sqlite::create_database(path_as_str)
                .await
                .expect("to create a Sqlite db");
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
        let sqlite_options = SqliteConnectOptions::from_str(path_as_str)
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

    async fn deployment(&self, id: &Ulid) -> Result<Deployment, DalError> {
        let row = sqlx::query("SELECT * FROM deployments WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(DalError::from)?
            .ok_or(DalError::DeploymentNotFound)?;
        Deployment::from_row(&row).map_err(DalError::Sqlx)
    }

    async fn insert_service_if_absent(&self, service: Service) -> Result<bool, DalError> {
        let Service {
            id,
            name,
            state_variant,
            state,
            last_update,
        } = service;

        if self.service(&id).await.is_ok() {
            return Ok(false);
        }

        sqlx::query("INSERT INTO services (id, name, state_variant, state, last_update) VALUES (?, ?, ?, ?, ?)")
            .bind(id.to_string())
            .bind(name)
            .bind(state_variant)
            .bind(last_update.timestamp())
            .bind(SqlxJson(state))
            .execute(&self.pool)
            .await?;
        Ok(true)
    }

    async fn insert_deployment(&self, deployment: Deployment) -> Result<(), DalError> {
        sqlx::query(
            "INSERT INTO deployments (id, service_id, last_update, is_next, git_commit_hash, git_commit_message, git_branch, git_dirty) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(deployment.id.to_string())
        .bind(deployment.service_id.to_string())
        .bind(deployment.last_update.timestamp())
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

    async fn running_deployments(&self) -> Result<Vec<RunningDeployment>, DalError> {
        sqlx::query_as(
            r#"SELECT d.id as id, s.name AS service_name, service_id, d.is_next as is_next, s.state as service_state
                    FROM deployments AS d
                    JOIN services AS s ON s.id = d.service_id
                    WHERE s.state_variant = ?
                    ORDER BY last_update"#,
        )
        .bind(ServiceRunning::name())
        .fetch_all(&self.pool)
        .await
        .map_err(DalError::from)
    }

    async fn update_service_state(
        &self,
        service_id: Ulid,
        state: ServiceState,
    ) -> Result<(), DalError> {
        let state_variant = state.to_string();
        query("UPDATE services SET state = ?1, state_variant = ?2, last_update = ?3 WHERE id = ?4")
            .bind(SqlxJson(state.clone()))
            .bind(state_variant)
            .bind(Utc::now().timestamp())
            .bind(service_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(DalError::from)
            .map(|_| ())
    }

    // Get services
    async fn services(&self) -> Result<Vec<Service>, DalError> {
        let services: Result<Vec<Service>, DalError> = query("SELECT * FROM services")
            .fetch_all(&self.pool)
            .await
            .map_err(DalError::Sqlx)?
            .iter()
            .map(|row| Service::from_row(row).map_err(DalError::Sqlx))
            .collect();
        services
    }
}

// User service model
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub id: Ulid,
    pub name: String,
    pub state_variant: String,
    pub state: ServiceState,
    pub last_update: DateTime<Utc>,
}

impl FromRow<'_, SqliteRow> for Service {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: Ulid::from_string(row.try_get("id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            name: row.try_get("name")?,
            state_variant: row.try_get("state_variant")?,
            state: row.try_get::<SqlxJson<ServiceState>, _>("state")?.0,
            last_update: DateTime::<Utc>::from_utc(
                NaiveDateTime::from_timestamp_opt(row.try_get("last_update")?, 0)
                    .expect("to get a naive date time out of the last_update field of the service"),
                Utc,
            ),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deployment {
    pub id: Ulid,
    pub service_id: Ulid,
    pub last_update: DateTime<Utc>,
    pub is_next: bool,
    pub git_commit_hash: Option<String>,
    pub git_commit_message: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

impl FromRow<'_, SqliteRow> for Deployment {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: Ulid::from_string(row.try_get("id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            service_id: Ulid::from_string(row.try_get("service_id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            last_update: DateTime::<Utc>::from_utc(
                NaiveDateTime::from_timestamp_opt(row.try_get("last_update")?, 0).expect(
                    "to get a naive date time out of the last_update field of the deployment",
                ),
                Utc,
            ),
            is_next: row.try_get("is_next")?,
            git_commit_hash: row.try_get("git_commit_hash")?,
            git_commit_message: row.try_get("git_commit_message")?,
            git_branch: row.try_get("git_branch")?,
            git_dirty: row.try_get("git_dirty")?,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RunningDeployment {
    pub id: Ulid,
    pub service_name: String,
    pub service_id: Ulid,
    pub is_next: bool,
    pub idle_minutes: u64,
}

impl FromRow<'_, SqliteRow> for RunningDeployment {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: Ulid::from_string(row.try_get("id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            service_name: row.try_get("service_name")?,
            service_id: Ulid::from_string(row.try_get("service_id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            is_next: row.try_get("is_next")?,
            idle_minutes: row
                .try_get::<SqlxJson<ServiceState>, _>("service_state")?
                .0
                .container()
                .map(|c| c.idle_minutes())
                .expect("to extract idle minutes from the service state"),
        })
    }
}
