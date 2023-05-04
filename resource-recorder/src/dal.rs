use crate::r#type::Type;
use async_trait::async_trait;
use sqlx::{migrate::Migrator, QueryBuilder, SqlitePool};
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
        let mut query_builder = QueryBuilder::new(
            "INSERT OR REPLACE INTO resources (id, service_id, type, config, data) ",
        );

        query_builder.push_values(resources, |mut b, mut resource| {
            if let Some(r_service_id) = resource.service_id {
                if r_service_id != service_id {
                    warn!("adding a resource that belongs to another service");
                }
            }

            // Make a new id for new resources
            if resource.id.is_none() {
                resource.id = Some(Uuid::new_v4());
            }

            b.push_bind(resource.id)
                .push_bind(service_id)
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
    pub id: Option<Uuid>,
    pub service_id: Option<Uuid>,
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
            id: None,
            service_id: None,
            r#type: Type::Database(crate::r#type::database::Type::Shared(
                crate::r#type::database::SharedType::Postgres,
            )),
            config: json!({"private": false}),
            data: json!({"username": "test"}),
        };
        let secrets = Resource {
            id: None,
            service_id: None,
            r#type: Type::Secrets,
            config: json!({}),
            data: json!({"password": "p@ssw0rd"}),
        };
        let static_folder = Resource {
            id: None,
            service_id: None,
            r#type: Type::StaticFolder,
            config: json!({"path": "static"}),
            data: json!({"path": "/tmp/static"}),
        };

        dal.add_resources(
            service_id,
            vec![database.clone(), secrets.clone(), static_folder.clone()],
        )
        .await
        .unwrap();

        let expected = vec![database, secrets, static_folder]
            .into_iter()
            .map(|mut r| {
                r.service_id = Some(service_id);
                r
            })
            .collect::<Vec<_>>();
        let actual = dal.get_resources(service_id).await.unwrap();
        let actual_without_id = actual
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r.id = None;
                r
            })
            .collect::<Vec<_>>();

        assert_eq!(expected, actual_without_id);
    }
}
