use crate::r#type::Type;
use async_trait::async_trait;
use sqlx::{migrate::Migrator, QueryBuilder, SqlitePool};
use uuid::Uuid;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[async_trait]
pub trait Dal {
    type Error: std::error::Error;

    /// Add a set of resources
    async fn add_resources(&self, resources: Vec<Resource>) -> Result<(), Self::Error>;

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

    async fn add_resources(&self, resources: Vec<Resource>) -> Result<(), Self::Error> {
        let mut query_builder = QueryBuilder::new(
            "INSERT OR REPLACE INTO resources (id, service_id, type, config, data) ",
        );

        query_builder.push_values(resources, |mut b, resource| {
            b.push_bind(resource.id)
                .push_bind(resource.service_id)
                .push_bind(resource.r#type)
                .push_bind(resource.config)
                .push_bind(resource.data);
        });

        query_builder.build().execute(&self.pool).await.map(|_| ())
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
    pub id: Uuid,
    pub service_id: Uuid,
    pub r#type: Type,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
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

        let database = Resource {
            id: Uuid::new_v4(),
            service_id,
            r#type: Type::Database(crate::r#type::database::Type::Shared(
                crate::r#type::database::SharedType::Postgres,
            )),
            config: json!({"private": false}),
            data: json!({"username": "test"}),
        };
        let secrets = Resource {
            id: Uuid::new_v4(),
            service_id,
            r#type: Type::Secrets,
            config: json!({}),
            data: json!({"password": "p@ssw0rd"}),
        };
        let static_folder = Resource {
            id: Uuid::new_v4(),
            service_id,
            r#type: Type::StaticFolder,
            config: json!({"path": "static"}),
            data: json!({"path": "/tmp/static"}),
        };

        dal.add_resources(vec![
            database.clone(),
            secrets.clone(),
            static_folder.clone(),
        ])
        .await
        .unwrap();

        let expected = vec![database, secrets, static_folder];
        let actual = dal.get_resources(service_id).await.unwrap();

        assert_eq!(expected, actual);
    }
}
