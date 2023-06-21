use std::{fmt, path::Path, str::FromStr};

use async_trait::async_trait;
use fqdn::Fqdn;
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    query, query_as,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow},
    FromRow, QueryBuilder, Row, SqlitePool,
};
use tracing::{error, info};

use crate::{acme::CustomDomain, AccountName, ProjectDetails, ProjectName};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(thiserror::Error, Debug)]
pub enum DalError {
    Sqlx(#[from] sqlx::Error),
    ProjectNotFound,
    CustomDomainNotFound,
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
            DalError::ProjectNotFound => "project not found",
            DalError::CustomDomainNotFound => "custom domain not found",
        };

        write!(f, "{msg}")
    }
}

#[async_trait]
pub trait Dal {
    /// Find project by project name.
    async fn get_project(&self, project_name: &ProjectName) -> Result<ProjectName, DalError>;
    /// Get all the projects in the gateway state.
    async fn get_all_projects(&self) -> Result<Vec<ProjectDetails>, DalError>;
    /// Get the name of all projects belonging to a user.
    async fn get_user_projects(
        &self,
        account_name: &AccountName,
    ) -> Result<Vec<ProjectName>, DalError>;
    /// Get the name of all projects belonging to a user.
    async fn get_user_projects_paginated(
        &self,
        account_name: &AccountName,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<ProjectName>, DalError>;
    /// Create a new custom domain.
    async fn create_custom_domain(
        &self,
        project_name: &ProjectName,
        fqdn: &Fqdn,
        certs: &str,
        private_key: &str,
    ) -> Result<(), DalError>;

    /// Get all custom domains.
    async fn get_custom_domains(&self) -> Result<Vec<CustomDomain>, DalError>;

    /// Find a custom domain for a specific project.
    async fn find_custom_domain_for_project(
        &self,
        project_name: &ProjectName,
    ) -> Result<CustomDomain, DalError>;

    /// Get project details for a custom domain.
    async fn project_details_for_custom_domain(
        &self,
        fqdn: &Fqdn,
    ) -> Result<CustomDomain, DalError>;
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
    // TODO: what else do we want to get from the project state?
    async fn get_project(&self, project_name: &ProjectName) -> Result<ProjectName, DalError> {
        let result = query("SELECT project_name FROM projects WHERE project_name=?1")
            .bind(project_name)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get("project_name"))
            .ok_or(DalError::ProjectNotFound)?;

        Ok(result)
    }

    async fn get_all_projects(&self) -> Result<Vec<ProjectDetails>, DalError> {
        let result = query("SELECT project_name, account_name FROM projects")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| ProjectDetails {
                project_name: row.try_get("project_name").unwrap(),
                account_name: row.try_get("account_name").unwrap(),
            })
            .collect();

        Ok(result)
    }

    // TODO: what else do we want to get from the project state?
    async fn get_user_projects_paginated(
        &self,
        account_name: &AccountName,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<ProjectName>, DalError> {
        let mut query =
            QueryBuilder::new("SELECT project_name FROM projects WHERE account_name = ");

        query
            .push_bind(account_name)
            .push(" ORDER BY created_at DESC NULLS LAST, project_name LIMIT ")
            .push_bind(limit);

        if offset > 0 {
            query.push(" OFFSET ").push_bind(offset);
        }

        let iter = query
            .build()
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| row.get("project_name"))
            .collect();

        Ok(iter)
    }

    // TODO: what else do we want to get from the project state?
    async fn get_user_projects(
        &self,
        account_name: &AccountName,
    ) -> Result<Vec<ProjectName>, DalError> {
        let iter = query("SELECT project_name FROM projects WHERE account_name = ?1")
            .bind(account_name)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| row.get("project_name"))
            .collect();

        Ok(iter)
    }

    async fn create_custom_domain(
        &self,
        project_name: &ProjectName,
        fqdn: &Fqdn,
        certs: &str,
        private_key: &str,
    ) -> Result<(), DalError> {
        query("INSERT OR REPLACE INTO custom_domains (fqdn, project_name, certificate, private_key) VALUES (?1, ?2, ?3, ?4)")
            .bind(fqdn.to_string())
            .bind(project_name)
            .bind(certs)
            .bind(private_key)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_custom_domains(&self) -> Result<Vec<CustomDomain>, DalError> {
        let result =
            query_as("SELECT fqdn, project_name, certificate, private_key FROM custom_domains")
                .fetch_all(&self.pool)
                .await?;

        Ok(result)
    }

    async fn find_custom_domain_for_project(
        &self,
        project_name: &ProjectName,
    ) -> Result<CustomDomain, DalError> {
        let custom_domain = query_as(
            "SELECT fqdn, project_name, certificate, private_key FROM custom_domains WHERE project_name = ?1",
        )
        .bind(project_name.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DalError::CustomDomainNotFound)?;

        Ok(custom_domain)
    }

    async fn project_details_for_custom_domain(
        &self,
        fqdn: &Fqdn,
    ) -> Result<CustomDomain, DalError> {
        let custom_domain = query_as(
            "SELECT fqdn, project_name, certificate, private_key FROM custom_domains WHERE fqdn = ?1",
        )
        .bind(fqdn.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DalError::CustomDomainNotFound)?;

        Ok(custom_domain)
    }
}

impl FromRow<'_, SqliteRow> for CustomDomain {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            fqdn: row.get::<&str, _>("fqdn").parse().unwrap(),
            project_name: row.try_get("project_name").unwrap(),
            certificate: row.get("certificate"),
            private_key: row.get("private_key"),
        })
    }
}
