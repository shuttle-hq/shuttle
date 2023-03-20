#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use shuttle_service::{database, error::CustomError, Error, Factory, ResourceBuilder};

#[cfg(feature = "postgres")]
pub struct Postgres {
    local_uri: Option<String>,
}

#[cfg(feature = "postgres")]
/// Get an `sqlx::PgPool` from any factory
#[async_trait]
impl ResourceBuilder<sqlx::PgPool> for Postgres {
    fn new() -> Self {
        Self { local_uri: None }
    }

    async fn build(self, factory: &mut dyn Factory) -> Result<sqlx::PgPool, Error> {
        let connection_string = match factory.get_environment() {
            shuttle_service::Environment::Production => {
                factory
                    .get_db_connection_string(database::Type::Shared(
                        database::SharedEngine::Postgres,
                    ))
                    .await?
            }
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.local_uri {
                    local_uri
                } else {
                    factory
                        .get_db_connection_string(database::Type::Shared(
                            database::SharedEngine::Postgres,
                        ))
                        .await?
                }
            }
        };

        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect(&connection_string)
            .await
            .map_err(CustomError::new)?;

        Ok(pool)
    }
}

#[cfg(feature = "postgres")]
impl Postgres {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.local_uri = Some(local_uri.to_string());

        self
    }
}

#[cfg(feature = "mongodb")]
pub struct MongoDb {
    local_uri: Option<String>,
}

/// Get a `mongodb::Database` from any factory
#[cfg(feature = "mongodb")]
#[async_trait]
impl ResourceBuilder<mongodb::Database> for MongoDb {
    fn new() -> Self {
        Self { local_uri: None }
    }

    async fn build(self, factory: &mut dyn Factory) -> Result<mongodb::Database, crate::Error> {
        let connection_string = match factory.get_environment() {
            shuttle_service::Environment::Production => factory
                .get_db_connection_string(database::Type::Shared(database::SharedEngine::MongoDb))
                .await
                .map_err(CustomError::new)?,
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.local_uri {
                    local_uri
                } else {
                    factory
                        .get_db_connection_string(database::Type::Shared(
                            database::SharedEngine::MongoDb,
                        ))
                        .await
                        .map_err(CustomError::new)?
                }
            }
        };

        let mut client_options = mongodb::options::ClientOptions::parse(connection_string)
            .await
            .map_err(CustomError::new)?;
        client_options.min_pool_size = Some(1);
        client_options.max_pool_size = Some(5);

        let client = mongodb::Client::with_options(client_options).map_err(CustomError::new)?;

        // Return a handle to the database defined at the end of the connection string, which is the users provisioned
        // database
        let database = client.default_database();

        match database {
            Some(database) => Ok(database),
            None => Err(crate::Error::Database(
                "mongodb connection string missing default database".into(),
            )),
        }
    }
}

#[cfg(feature = "mongodb")]
impl MongoDb {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.local_uri = Some(local_uri.to_string());

        self
    }
}
