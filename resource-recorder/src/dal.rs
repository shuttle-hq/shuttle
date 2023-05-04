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
        service_id: Uuid,
        resources: Vec<Resource>,
    ) -> Result<(), Self::Error>;

    /// Get the resources that belong to a service
    async fn get_resources(&self, service_id: Uuid) -> Result<Vec<Resource>, Self::Error>;
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
        service_id: Uuid,
        resources: Vec<Resource>,
    ) -> Result<(), Self::Error> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("UPDATE resources SET is_active = false WHERE service_id = ?")
            .bind(service_id)
            .execute(&mut transaction)
            .await?;

        for mut resource in resources {
            if let Some(r_service_id) = resource.service_id {
                if r_service_id != service_id {
                    warn!("adding a resource that belongs to another service");
                }
            }

            // Make a new id for new resources
            if resource.id.is_none() {
                resource.id = Some(Uuid::new_v4());
            }

            sqlx::query("INSERT OR REPLACE INTO resources (id, service_id, type, config, data, is_active) VALUES(?, ?, ?, ?, ?, ?)")
            .bind(resource.id)
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

    async fn get_resources(&self, service_id: Uuid) -> Result<Vec<Resource>, Self::Error> {
        sqlx::query_as(r#"SELECT * FROM resources WHERE service_id = ?"#)
            .bind(service_id)
            .fetch_all(&self.pool)
            .await
    }
}

#[derive(sqlx::FromRow, Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub r#type: Type,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl Resource {
    fn new(r#type: Type, data: serde_json::Value, config: serde_json::Value) -> Self {
        Self {
            id: None,
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

        dal.add_resources(service_id, vec![database.clone(), static_folder.clone()])
            .await
            .unwrap();

        let actual = dal.get_resources(service_id).await.unwrap();

        // The query would set these
        database.id = actual[0].id;
        database.created_at = actual[0].created_at;
        static_folder.id = actual[1].id;
        static_folder.created_at = actual[1].created_at;

        let expected = vec![database.clone(), static_folder]
            .into_iter()
            .map(|mut r| {
                r.service_id = Some(service_id);
                r
            })
            .collect::<Vec<_>>();

        assert_eq!(expected, actual);

        // This time the user is adding secrets but dropping the static folders
        let mut secrets = Resource::new(Type::Secrets, json!({}), json!({"password": "p@ssw0rd"}));

        let database = actual[0].clone();
        let mut static_folder = actual[1].clone();

        dal.add_resources(service_id, vec![database.clone(), secrets.clone()])
            .await
            .unwrap();

        let actual = dal.get_resources(service_id).await.unwrap();

        static_folder.is_active = false;
        secrets.id = actual[1].id;
        secrets.created_at = actual[1].created_at;

        let expected = vec![database, secrets, static_folder]
            .into_iter()
            .map(|mut r| {
                r.service_id = Some(service_id);
                r
            })
            .collect::<Vec<_>>();

        assert_eq!(expected, actual);
    }
}
