#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use shuttle_service::{database, error::CustomError, Error, Factory, ResourceBuilder};

#[cfg(feature = "postgres")]
pub struct Postgres;

#[cfg(feature = "postgres")]
/// Get an `sqlx::PgPool` from any factory
#[async_trait]
impl ResourceBuilder<sqlx::PgPool> for Postgres {
    fn new() -> Self {
        Self {}
    }

    async fn build(self, factory: &mut dyn Factory) -> Result<sqlx::PgPool, Error> {
        let connection_string = factory
            .get_db_connection_string(database::Type::Shared(database::SharedEngine::Postgres))
            .await?;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect(&connection_string)
            .await
            .map_err(CustomError::new)?;

        Ok(pool)
    }
}

#[cfg(feature = "mongodb")]
pub struct MongoDb;

/// Get a `mongodb::Database` from any factory
#[cfg(feature = "mongodb")]
#[async_trait]
impl ResourceBuilder<mongodb::Database> for MongoDb {
    fn new() -> Self {
        Self {}
    }

    async fn build(self, factory: &mut dyn Factory) -> Result<mongodb::Database, crate::Error> {
        let connection_string = factory
            .get_db_connection_string(database::Type::Shared(database::SharedEngine::MongoDb))
            .await
            .map_err(CustomError::new)?;

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
