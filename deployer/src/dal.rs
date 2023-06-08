use std::fmt;
use std::path::Path;
use std::str::FromStr;

use axum::async_trait;
use futures::future::ok;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::types::Json as SqlxJson;
use sqlx::{migrate::Migrator, query, Row, SqlitePool};
use thiserror::Error;
use tracing::{error, info};

use crate::account::AccountName;
use crate::project::machine::Project;
use crate::project::name::ProjectName;

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
    async fn project_state(&self, project_name: &ProjectName) -> Result<Project, DalError>;
    /// Fetch the account name of a project.
    async fn account(&self, project_name: &ProjectName) -> Result<AccountName, DalError>;
    /// Update the project information.
    async fn update_project(
        &self,
        project_name: &ProjectName,
        project: &Project,
    ) -> Result<(), DalError>;
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
    async fn project_state(&self, project_name: &ProjectName) -> Result<Project, DalError> {
        Ok(
            query("SELECT project_state FROM projects WHERE project_id=?1")
                .bind(project_name.to_string())
                .fetch_optional(&self.pool)
                .await?
                .ok_or(DalError::ProjectNotFound)?
                .try_get::<SqlxJson<Project>, _>("project_state")
                .map_err(|err| DalError::Sqlx(err))?
                .0,
        )
    }

    async fn account(&self, project_name: &ProjectName) -> Result<AccountName, DalError> {
        Ok(
            query("SELECT account_name FROM projects WHERE project_name = ?1")
                .bind(project_name.to_string())
                .fetch_optional(&self.pool)
                .await?
                .ok_or(DalError::ProjectNotFound)?
                .get("account_name"),
        )
    }

    async fn update_project(
        &self,
        project_name: &ProjectName,
        project: &Project,
    ) -> Result<(), DalError> {
        let query = match project {
            Project::Creating(state) => query(
                "UPDATE projects SET initial_key = ?1, project_state = ?2 WHERE project_name = ?3",
            )
            .bind(state.initial_key())
            .bind(SqlxJson(project))
            .bind(project_name),
            _ => query("UPDATE projects SET project_state = ?1 WHERE project_name = ?2")
                .bind(SqlxJson(project))
                .bind(project_name),
        };

        query
            .execute(&self.pool)
            .await
            .map_err(|err| DalError::Sqlx(err))?;

        Ok(())
    }
}

// query("SELECT account_name FROM projects WHERE project_name = ?1")
// .bind(project_name)
// .fetch_optional(&self.db)
// .await?
// .map(|row| row.get("account_name"))
// .ok_or_else(|| Error::from(ErrorKind::ProjectNotFound))

// impl FromRow<'_, SqliteRow> for Project {
//     fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
//         Ok(Self {
//             project_id: Some(
//                 Ulid::from_string(row.try_get("project_id")?)
//                     .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
//             ),
//             project_name: Some(row.try_get("service_id")?)
//                 .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
//             r#type: row.try_get("type")?,
//             data: row.try_get("data")?,
//             config: row.try_get("config")?,
//             is_active: row.try_get("is_active")?,
//             created_at: row.try_get("created_at")?,
//             last_updated: row.try_get("last_updated")?,
//         })
//     }
// }

// #[derive(Clone, Debug, Eq, PartialEq)]
// pub struct Resource {
//     project_id: Option<Ulid>,
//     service_id: Option<Ulid>,
//     r#type: Type,
//     data: Vec<u8>,
//     config: Vec<u8>,
//     is_active: bool,
//     created_at: DateTime<Utc>,
//     last_updated: DateTime<Utc>,
// }

// impl FromRow<'_, SqliteRow> for Resource {
//     fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
//         Ok(Self {
//             project_id: Some(
//                 Ulid::from_string(row.try_get("project_id")?)
//                     .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
//             ),
//             service_id: Some(
//                 Ulid::from_string(row.try_get("service_id")?)
//                     .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
//             ),
//             r#type: row.try_get("type")?,
//             data: row.try_get("data")?,
//             config: row.try_get("config")?,
//             is_active: row.try_get("is_active")?,
//             created_at: row.try_get("created_at")?,
//             last_updated: row.try_get("last_updated")?,
//         })
//     }
// }

// impl TryFrom<record_request::Resource> for Resource {
//     type Error = String;

//     fn try_from(value: record_request::Resource) -> Result<Self, Self::Error> {
//         Ok(Self::new(value.r#type.parse()?, value.data, value.config))
//     }
// }

// impl From<Resource> for resource_recorder::Resource {
//     fn from(value: Resource) -> Self {
//         Self {
//             project_id: value
//                 .project_id
//                 .expect("row to have a project id")
//                 .to_string(),
//             service_id: value
//                 .service_id
//                 .expect("row to have a service id")
//                 .to_string(),
//             r#type: value.r#type.to_string(),
//             config: value.config,
//             data: value.data,
//             is_active: value.is_active,
//             created_at: Some(Timestamp::from(SystemTime::from(value.created_at))),
//             last_updated: Some(Timestamp::from(SystemTime::from(value.last_updated))),
//         }
//     }
// }

// impl TryFrom<resource_recorder::Resource> for Resource {
//     type Error = Error;

//     fn try_from(value: resource_recorder::Resource) -> Result<Self, Self::Error> {
//         Ok(Self {
//             project_id: Some(value.project_id.parse()?),
//             service_id: Some(value.service_id.parse()?),
//             r#type: value.r#type.parse()?,
//             data: value.data,
//             config: value.config,
//             is_active: value.is_active,
//             created_at: DateTime::from(SystemTime::try_from(value.created_at.unwrap_or_default())?),
//             last_updated: DateTime::from(SystemTime::try_from(
//                 value.last_updated.unwrap_or_default(),
//             )?),
//         })
//     }
// }

// impl Resource {
//     /// Create a new type of resource
//     fn new(r#type: Type, data: Vec<u8>, config: Vec<u8>) -> Self {
//         Self {
//             project_id: None,
//             service_id: None,
//             r#type,
//             data,
//             config,
//             is_active: true,
//             created_at: Default::default(),
//             last_updated: Default::default(),
//         }
//     }
// }
