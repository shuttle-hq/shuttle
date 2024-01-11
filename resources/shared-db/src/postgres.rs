use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database, resource::Type, DatabaseResource, DbInput, Error, Factory, IntoResource,
    ResourceBuilder,
};

/// Handles the state of a Shuttle managed Postgres DB and sets up a Postgres driver.
#[derive(Serialize)]
pub struct Postgres {
    config: DbInput,
}

impl Postgres {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.config.local_uri = Some(local_uri.to_string());

        self
    }
}

/// Get a Postgres Database as an `sqlx::PgPool` or connection string
#[async_trait]
impl ResourceBuilder for Postgres {
    const TYPE: Type = Type::Database(database::Type::Shared(database::SharedEngine::Postgres));

    type Config = DbInput;

    type Output = Test;

    fn new() -> Self {
        Self {
            config: Default::default(),
        }
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        let info = match factory.get_metadata().env {
            shuttle_service::Environment::Deployment => DatabaseResource::Info(
                factory
                    .get_db_connection(database::Type::Shared(database::SharedEngine::Postgres))
                    .await?,
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.config.local_uri {
                    DatabaseResource::ConnectionString(local_uri)
                } else {
                    DatabaseResource::Info(
                        factory
                            .get_db_connection(database::Type::Shared(
                                database::SharedEngine::Postgres,
                            ))
                            .await?,
                    )
                }
            }
        };

        Ok(Test(info))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Test(DatabaseResource);

#[cfg(feature = "sqlx")]
#[async_trait]
impl IntoResource for Test {
    type Output = sqlx::PgPool;

    async fn init(self) -> Result<Self::Output, Error> {
        let connection_string = match self.0 {
            DatabaseResource::ConnectionString(s) => s.clone(),
            DatabaseResource::Info(info) => info.connection_string_shuttle(),
        };

        Ok(sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect(&connection_string)
            .await
            .map_err(shuttle_service::error::CustomError::new)?)
    }
}

// #[cfg(not(feature = "sqlx"))]
// #[async_trait]
// impl FromResource<DatabaseResource> for String {
//     async fn init(resource: DatabaseResource) -> Result<Self, Error> {
//         Ok(match resource {
//             DatabaseResource::ConnectionString(s) => s.clone(),
//             DatabaseResource::Info(info) => info.connection_string_shuttle(),
//         })
//     }
// }
