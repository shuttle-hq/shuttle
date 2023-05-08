use crate::r#type::Type;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{migrate::Migrator, SqlitePool};
use tracing::warn;
use uuid::Uuid;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[async_trait]
pub trait Dal {
    type Error: std::error::Error;

    /// Add a set of resources for a service
    async fn add_resources(
        &self,
        project_id: Uuid,
        service_id: Uuid,
        resources: Vec<Resource>,
    ) -> Result<(), Self::Error>;

    /// Get the resources that belong to a project
    async fn get_project_resources(&self, project_id: Uuid) -> Result<Vec<Resource>, Self::Error>;

    /// Get the resources that belong to a service
    async fn get_service_resources(&self, service_id: Uuid) -> Result<Vec<Resource>, Self::Error>;
}

pub struct Sqlite {
    pool: SqlitePool,
}

impl Sqlite {
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
        project_id: Uuid,
        service_id: Uuid,
        resources: Vec<Resource>,
    ) -> Result<(), Self::Error> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("UPDATE resources SET is_active = false WHERE service_id = ?")
            .bind(service_id)
            .execute(&mut transaction)
            .await?;

        // Making mutliple DB "connections" is fine since the sqlite is on the same machine
        for mut resource in resources {
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
            .bind(project_id)
            .bind(service_id)
            .bind(resource.r#type)
            .bind(resource.config)
            .bind(resource.data)
            .bind(resource.is_active)
            .execute(&mut transaction)
            .await?;
        }

        transaction.commit().await
    }

    async fn get_project_resources(&self, project_id: Uuid) -> Result<Vec<Resource>, Self::Error> {
        sqlx::query_as(r#"SELECT * FROM resources WHERE project_id = ?"#)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
    }

    async fn get_service_resources(&self, service_id: Uuid) -> Result<Vec<Resource>, Self::Error> {
        sqlx::query_as(r#"SELECT * FROM resources WHERE service_id = ?"#)
            .bind(service_id)
            .fetch_all(&self.pool)
            .await
    }
}

#[derive(sqlx::FromRow, Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub project_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub r#type: Type,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl Resource {
    /// Create a new type of resource
    fn new(r#type: Type, data: serde_json::Value, config: serde_json::Value) -> Self {
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
    use uuid::Uuid;

    use crate::{
        dal::{Dal, Resource},
        r#type::Type,
    };

    use super::Sqlite;

    #[tokio::test]
    async fn manage_resources() {
        let dal = Sqlite::new_in_memory().await;
        let project_id = Uuid::new_v4();
        let service_id = Uuid::new_v4();

        // Test with a small set of initial resources
        let mut database = Resource::new(
            Type::Database(crate::r#type::database::Type::Shared(
                crate::r#type::database::SharedType::Postgres,
            )),
            json!({"private": false}),
            json!({"username": "test"}),
        );
        let mut static_folder = Resource::new(
            Type::StaticFolder,
            json!({"path": "static"}),
            json!({"path": "/tmp/static"}),
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
        let mut secrets = Resource::new(Type::Secrets, json!({}), json!({"password": "p@ssw0rd"}));

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
        database.data = json!({"private": true});

        dal.add_resources(project_id, service_id, vec![database.clone()])
            .await
            .unwrap();

        let actual = dal.get_service_resources(service_id).await.unwrap();

        // The query would set this
        secrets.is_active = false;

        let expected = vec![static_folder.clone(), secrets.clone(), database.clone()];

        assert_eq!(expected, actual);

        // Add resources to another service in the same project
        let service_id2 = Uuid::new_v4();
        let mut secrets2 = Resource::new(Type::Secrets, json!({}), json!({"token": "12345"}));

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
        let expected = vec![database, secrets, static_folder, secrets2];

        assert_eq!(expected, actual);

        // Add resources to another project
        let project_id2 = Uuid::new_v4();
        let service_id3 = Uuid::new_v4();
        let mut static_folder2 = Resource::new(
            Type::StaticFolder,
            json!({"path": "public"}),
            json!({"path": "/tmp/public"}),
        );

        dal.add_resources(project_id2, service_id3, vec![static_folder2.clone()])
            .await
            .unwrap();

        let actual = dal.get_service_resources(service_id3).await.unwrap();

        // The query would set these
        static_folder2.project_id = Some(project_id2);
        static_folder2.service_id = Some(service_id3);
        static_folder2.created_at = actual[0].created_at;

        let expected = vec![static_folder2];

        assert_eq!(expected, actual);

        let actual = dal.get_project_resources(project_id2).await.unwrap();
        assert_eq!(expected, actual);
    }
}
