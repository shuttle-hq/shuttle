use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database, error::CustomError, resource::Type, DatabaseResource, DbInput, Error, Factory,
    IntoResource, ResourceBuilder,
};

/// Shuttle managed MongoDB in a shared cluster
#[derive(Default)]
pub struct MongoDb(DbInput);

impl MongoDb {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.0.local_uri = Some(local_uri.to_string());

        self
    }
}

#[async_trait]
impl ResourceBuilder for MongoDb {
    const TYPE: Type = Type::Database(database::Type::Shared(database::SharedEngine::MongoDb));

    type Config = DbInput;

    type Output = Wrap;

    fn config(&self) -> &Self::Config {
        &self.0
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        let info = match factory.get_metadata().env {
            shuttle_service::Environment::Deployment => DatabaseResource::Info(
                factory
                    .get_db_connection(database::Type::Shared(database::SharedEngine::MongoDb))
                    .await
                    .map_err(CustomError::new)?,
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.0.local_uri {
                    DatabaseResource::ConnectionString(local_uri)
                } else {
                    DatabaseResource::Info(
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

#[async_trait]
impl IntoResource<mongodb::Database> for Wrap {
    async fn into_resource(self) -> Result<mongodb::Database, Error> {
        let connection_string = match self.0 {
            DatabaseResource::ConnectionString(s) => s.clone(),
            DatabaseResource::Info(info) => info.connection_string_shuttle(),
        };

        let mut client_options = mongodb::options::ClientOptions::parse(connection_string)
            .await
            .map_err(CustomError::new)?;
        client_options.min_pool_size = Some(1);
        client_options.max_pool_size = Some(5);

        let client = mongodb::Client::with_options(client_options).map_err(CustomError::new)?;

        // Return a handle to the database defined at the end of the connection string,
        // which is the users provisioned database
        let database = client.default_database();

        database.ok_or_else(|| {
            Error::Database("mongodb connection string missing default database".into())
        })
    }
}
