use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database, resource::Type, DatabaseResource, DbInput, Error, Factory, IntoResource,
    ResourceBuilder,
};

/// Shuttle managed Postgres DB in a shared cluster
#[derive(Default)]
pub struct Postgres(DbInput);

impl Postgres {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.0.local_uri = Some(local_uri.to_string());

        self
    }
}

#[async_trait]
impl ResourceBuilder for Postgres {
    const TYPE: Type = Type::Database(database::Type::Shared(database::SharedEngine::Postgres));

    type Config = DbInput;

    type Output = Wrap;

    fn config(&self) -> &Self::Config {
        &self.0
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        let info = match factory.get_metadata().env {
            shuttle_service::Environment::Deployment => DatabaseResource::Info(
                factory
                    .get_db_connection(database::Type::Shared(database::SharedEngine::Postgres))
                    .await?,
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.0.local_uri {
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

        Ok(Wrap(info))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Wrap(DatabaseResource);

#[async_trait]
impl IntoResource<String> for Wrap {
    async fn into_resource(self) -> Result<String, Error> {
        Ok(match self.0 {
            DatabaseResource::ConnectionString(s) => s.clone(),
            DatabaseResource::Info(info) => info.connection_string_shuttle(),
        })
    }
}

#[cfg(feature = "sqlx")]
#[async_trait]
impl IntoResource<sqlx::PgPool> for Wrap {
    async fn into_resource(self) -> Result<sqlx::PgPool, Error> {
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
