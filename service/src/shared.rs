use tokio::runtime::Runtime;

use crate::{database, error::CustomError, Factory, ResourceBuilder};
use async_trait::async_trait;

#[cfg(feature = "sqlx-postgres")]
pub struct Postgres;

/// Get an `sqlx::PgPool` from any factory
#[cfg(feature = "sqlx-postgres")]
#[async_trait]
impl ResourceBuilder<sqlx::PgPool> for Postgres {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        runtime: &Runtime,
    ) -> Result<sqlx::PgPool, crate::Error> {
        let connection_string = factory
            .get_db_connection_string(database::Type::Shared(database::SharedEngine::Postgres))
            .await?;

        // A sqlx Pool cannot cross runtime boundaries, so make sure to create the Pool on the service end
        let pool = runtime
            .spawn(async move {
                sqlx::postgres::PgPoolOptions::new()
                    .min_connections(1)
                    .max_connections(5)
                    .connect(&connection_string)
                    .await
            })
            .await
            .map_err(CustomError::new)?
            .map_err(CustomError::new)?;

        Ok(pool)
    }
}

#[cfg(feature = "mongodb-integration")]
pub struct MongoDb;

/// Get a `mongodb::Database` from any factory
#[cfg(feature = "mongodb-integration")]
#[async_trait]
impl ResourceBuilder<mongodb::Database> for MongoDb {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        runtime: &Runtime,
    ) -> Result<mongodb::Database, crate::Error> {
        let connection_string = factory
            .get_db_connection_string(database::Type::Shared(database::SharedEngine::MongoDb))
            .await
            .map_err(CustomError::new)?;

        let mut client_options = mongodb::options::ClientOptions::parse(connection_string)
            .await
            .map_err(CustomError::new)?;
        client_options.min_pool_size = Some(1);
        client_options.max_pool_size = Some(5);

        // A mongodb client cannot cross runtime boundaries, so make sure to create the client on the service end
        let client = runtime
            .spawn(async move { mongodb::Client::with_options(client_options) })
            .await
            .map_err(CustomError::new)?
            .map_err(CustomError::new)?;

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
