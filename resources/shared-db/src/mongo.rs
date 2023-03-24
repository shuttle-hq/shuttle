use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{database, error::CustomError, Error, Factory, ResourceBuilder, Type};

use crate::SharedDbOutput;

#[derive(Serialize)]
pub struct MongoDb {
    local_uri: Option<String>,
}

/// Get a `mongodb::Database` from any factory
#[async_trait]
impl ResourceBuilder<mongodb::Database> for MongoDb {
    const TYPE: Type = Type::Database(database::Type::Shared(database::SharedEngine::MongoDb));

    type Output = SharedDbOutput;

    fn new() -> Self {
        Self { local_uri: None }
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        let info = match factory.get_environment() {
            shuttle_service::Environment::Production => SharedDbOutput::Shared(
                factory
                    .get_db_connection(database::Type::Shared(database::SharedEngine::MongoDb))
                    .await
                    .map_err(CustomError::new)?,
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.local_uri {
                    SharedDbOutput::Local(local_uri)
                } else {
                    SharedDbOutput::Shared(
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

    async fn build(build_data: &Self::Output) -> Result<mongodb::Database, Error> {
        let connection_string = match build_data {
            SharedDbOutput::Local(local_uri) => local_uri.clone(),
            SharedDbOutput::Shared(info) => info.connection_string_private(),
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
            None => Err(Error::Database(
                "mongodb connection string missing default database".into(),
            )),
        }
    }
}

impl MongoDb {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.local_uri = Some(local_uri.to_string());

        self
    }
}
