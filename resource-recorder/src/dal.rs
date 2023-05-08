use std::{path::Path, str::FromStr};

use crate::r#type::Type;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shuttle_proto::resource_recorder::record_request;
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow},
    FromRow, Row, SqlitePool,
};
use tracing::{info, warn};
use ulid::Ulid;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[async_trait]
pub trait Dal {
    type Error: std::error::Error + Send;

    /// Add a set of resources for a service
    async fn add_resources(
        &self,
        project_id: Ulid,
        service_id: Ulid,
        resources: Vec<Resource>,
    ) -> Result<(), Self::Error>;

    /// Get the resources that belong to a project
    async fn get_project_resources(&self, project_id: Ulid) -> Result<Vec<Resource>, Self::Error>;

    /// Get the resources that belong to a service
    async fn get_service_resources(&self, service_id: Ulid) -> Result<Vec<Resource>, Self::Error>;

    /// Delete a resource
    async fn delete_resource(&self, resource: &Resource) -> Result<(), Self::Error>;
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
    async fn new_in_memory() -> Self {
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
    type Error = sqlx::Error;

    async fn add_resources(
        &self,
        project_id: Ulid,
        service_id: Ulid,
        resources: Vec<Resource>,
    ) -> Result<(), Self::Error> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("UPDATE resources SET is_active = false WHERE service_id = ?")
            .bind(service_id.to_string())
            .execute(&mut transaction)
            .await?;

        // Making mutliple DB "connections" is fine since the sqlite is on the same machine
        for resource in resources {
            if let Some(r_project_id) = resource.project_id {
                if r_project_id != project_id {
                    warn!("adding a resource that belongs to another project");
                }
            }

            if let Some(r_service_id) = resource.service_id {
                if r_service_id != service_id {
                    warn!("adding a resource that belongs to another service");
                }
            }

            sqlx::query("INSERT OR REPLACE INTO resources (project_id, service_id, type, config, data, is_active) VALUES(?, ?, ?, ?, ?, ?)")
            .bind(project_id.to_string())
            .bind(service_id.to_string())
            .bind(resource.r#type)
            .bind(resource.config)
            .bind(resource.data)
            .bind(resource.is_active)
            .execute(&mut transaction)
            .await?;
        }

        transaction.commit().await
    }

    async fn get_project_resources(&self, project_id: Ulid) -> Result<Vec<Resource>, Self::Error> {
        sqlx::query_as(r#"SELECT * FROM resources WHERE project_id = ?"#)
            .bind(project_id.to_string())
            .fetch_all(&self.pool)
            .await
    }

    async fn get_service_resources(&self, service_id: Ulid) -> Result<Vec<Resource>, Self::Error> {
        sqlx::query_as(r#"SELECT * FROM resources WHERE service_id = ?"#)
            .bind(service_id.to_string())
            .fetch_all(&self.pool)
            .await
    }

    async fn delete_resource(&self, resource: &Resource) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM resources WHERE project_id = ? AND service_id = ? AND type = ?")
            .bind(resource.project_id.map(|u| u.to_string()))
            .bind(resource.service_id.map(|u| u.to_string()))
            .bind(resource.r#type)
            .execute(&self.pool)
            .await
            .map(|_| ())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub project_id: Option<Ulid>,
    pub service_id: Option<Ulid>,
    pub r#type: Type,
    pub data: Vec<u8>,
    pub config: Vec<u8>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl FromRow<'_, SqliteRow> for Resource {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            project_id: Some(
                Ulid::from_string(row.try_get("project_id")?)
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            ),
            service_id: Some(
                Ulid::from_string(row.try_get("service_id")?)
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            ),
            r#type: row.try_get("type")?,
            data: row.try_get("data")?,
            config: row.try_get("config")?,
            is_active: row.try_get("is_active")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl TryFrom<record_request::Resource> for Resource {
    type Error = String;

    fn try_from(value: record_request::Resource) -> Result<Self, Self::Error> {
        Ok(Self::new(value.r#type.parse()?, value.data, value.config))
    }
}

impl Resource {
    /// Create a new type of resource
    fn new(r#type: Type, data: Vec<u8>, config: Vec<u8>) -> Self {
        Self {
            project_id: None,
            service_id: None,
            r#type,
            data,
            config,
            is_active: true,
            created_at: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use ulid::Ulid;

    use crate::{
        dal::{Dal, Resource},
        r#type::Type,
    };

    use super::Sqlite;

    #[tokio::test]
    async fn manage_resources() {
        let dal = Sqlite::new_in_memory().await;
        let project_id = Ulid::new();
        let service_id = Ulid::new();

        // Test with a small set of initial resources
        let mut database = Resource::new(
            Type::Database(crate::r#type::database::Type::Shared(
                crate::r#type::database::SharedType::Postgres,
            )),
            serde_json::to_vec(&json!({"private": false})).unwrap(),
            serde_json::to_vec(&json!({"username": "test"})).unwrap(),
        );
        let mut static_folder = Resource::new(
            Type::StaticFolder,
            serde_json::to_vec(&json!({"path": "static"})).unwrap(),
            serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
        );

        dal.add_resources(
            project_id,
            service_id,
            vec![database.clone(), static_folder.clone()],
        )
        .await
        .unwrap();

        let actual = dal.get_service_resources(service_id).await.unwrap();

        // The query would set these
        database.project_id = actual[0].project_id;
        database.service_id = actual[0].service_id;
        database.created_at = actual[0].created_at;
        static_folder.project_id = actual[1].project_id;
        static_folder.service_id = actual[1].service_id;
        static_folder.created_at = actual[1].created_at;

        let expected = vec![database.clone(), static_folder];

        assert_eq!(expected, actual);

        // This time the user is adding secrets but dropping the static folders
        let mut secrets = Resource::new(
            Type::Secrets,
            serde_json::to_vec(&json!({})).unwrap(),
            serde_json::to_vec(&json!({"password": "p@ssw0rd"})).unwrap(),
        );

        let mut database = actual[0].clone();
        let mut static_folder = actual[1].clone();

        dal.add_resources(
            project_id,
            service_id,
            vec![database.clone(), secrets.clone()],
        )
        .await
        .unwrap();

        let actual = dal.get_service_resources(service_id).await.unwrap();

        // The query would set these
        static_folder.is_active = false;
        secrets.project_id = actual[2].project_id;
        secrets.service_id = actual[2].service_id;
        secrets.created_at = actual[2].created_at;

        let expected = vec![static_folder.clone(), database.clone(), secrets.clone()];

        assert_eq!(expected, actual);

        // This time the user is using only the database with updates
        database.data = serde_json::to_vec(&json!({"private": true})).unwrap();

        dal.add_resources(project_id, service_id, vec![database.clone()])
            .await
            .unwrap();

        let actual = dal.get_service_resources(service_id).await.unwrap();

        // The query would set this
        secrets.is_active = false;

        let expected = vec![static_folder.clone(), secrets.clone(), database.clone()];

        assert_eq!(expected, actual);

        // Add resources to another service in the same project
        let service_id2 = Ulid::new();
        let mut secrets2 = Resource::new(
            Type::Secrets,
            serde_json::to_vec(&json!({})).unwrap(),
            serde_json::to_vec(&json!({"token": "12345"})).unwrap(),
        );

        dal.add_resources(project_id, service_id2, vec![secrets2.clone()])
            .await
            .unwrap();

        let actual = dal.get_service_resources(service_id2).await.unwrap();

        // The query would set these
        secrets2.project_id = Some(project_id);
        secrets2.service_id = Some(service_id2);
        secrets2.created_at = actual[0].created_at;

        let expected = vec![secrets2.clone()];

        assert_eq!(expected, actual);

        let actual = dal.get_project_resources(project_id).await.unwrap();
        let expected = vec![static_folder, secrets, database, secrets2];

        assert_eq!(expected, actual);

        // Add resources to another project
        let project_id2 = Ulid::new();
        let service_id3 = Ulid::new();
        let mut static_folder2 = Resource::new(
            Type::StaticFolder,
            serde_json::to_vec(&json!({"path": "public"})).unwrap(),
            serde_json::to_vec(&json!({"path": "/tmp/public"})).unwrap(),
        );

        dal.add_resources(project_id2, service_id3, vec![static_folder2.clone()])
            .await
            .unwrap();

        let actual = dal.get_service_resources(service_id3).await.unwrap();

        // The query would set these
        static_folder2.project_id = Some(project_id2);
        static_folder2.service_id = Some(service_id3);
        static_folder2.created_at = actual[0].created_at;

        let expected = vec![static_folder2.clone()];

        assert_eq!(expected, actual);

        let actual = dal.get_project_resources(project_id2).await.unwrap();
        assert_eq!(expected, actual);

        // Deleting a resource
        dal.delete_resource(&static_folder2).await.unwrap();

        let actual = dal.get_service_resources(service_id3).await.unwrap();
        assert!(
            actual.is_empty(),
            "service should have no resources after deletion: {actual:?}"
        );

        let actual = dal.get_project_resources(project_id2).await.unwrap();
        assert!(
            actual.is_empty(),
            "project should have no resources after deletion: {actual:?}"
        );
    }
}
