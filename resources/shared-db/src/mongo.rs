use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{
    database, error::CustomError, DbInput, DbOutput, Error, Factory, ResourceBuilder, Type,
};

// Return different things based on feature choice
#[cfg(feature = "mongodb-client")]
type ReturnType = mongodb::Database;
#[cfg(not(feature = "mongodb-client"))]
type ReturnType = String;

#[derive(Serialize)]
pub struct MongoDb {
    config: DbInput,
}

impl MongoDb {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.config.local_uri = Some(local_uri.to_string());

        self
    }
}

/// Get a MongoDB database connection or connection string
#[async_trait]
impl ResourceBuilder<ReturnType> for MongoDb {
    const TYPE: Type = Type::Database(database::Type::Shared(database::SharedEngine::MongoDb));

    type Config = DbInput;

    type Output = DbOutput;

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
            shuttle_service::Environment::Deployment => DbOutput::Info(
                factory
                    .get_db_connection(database::Type::Shared(database::SharedEngine::MongoDb))
                    .await
                    .map_err(CustomError::new)?,
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.config.local_uri {
                    DbOutput::Local(local_uri)
                } else {
                    DbOutput::Info(
                        factory
                            .get_db_connection(database::Type::Shared(
                                database::SharedEngine::MongoDb,
                            ))
                            .await
                            .map_err(CustomError::new)?,
                    )
                }
            }
        };
        Ok(info)
    }

    async fn build(build_data: &Self::Output) -> Result<ReturnType, Error> {
        let connection_string = match build_data {
            DbOutput::Local(local_uri) => local_uri.clone(),
            DbOutput::Info(info) => info.connection_string_private(),
        };

        #[cfg(feature = "mongodb-client")]
        return {
            let mut client_options = mongodb::options::ClientOptions::parse(connection_string)
                .await
                .map_err(CustomError::new)?;
            client_options.min_pool_size = Some(1);
            client_options.max_pool_size = Some(5);

            let client = mongodb::Client::with_options(client_options).map_err(CustomError::new)?;

            // Return a handle to the database defined at the end of the connection string, which is the users provisioned
            // database
            let database = client.default_database();

            database.ok_or_else(|| {
                Error::Database("mongodb connection string missing default database".into())
            })
        };

        #[cfg(not(feature = "mongodb-client"))]
        return Ok(connection_string);
    }
}
