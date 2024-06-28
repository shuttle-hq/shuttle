use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database,
    resource::{ProvisionResourceRequest, ShuttleResourceOutput, Type},
    DatabaseResource, DbInput, Error, IntoResource, ResourceFactory, ResourceInputBuilder,
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
impl ResourceInputBuilder for MongoDb {
    type Input = ProvisionResourceRequest;
    type Output = OutputWrapper;

    async fn build(self, _factory: &ResourceFactory) -> Result<Self::Input, Error> {
        Ok(ProvisionResourceRequest::new(
            Type::Database(database::Type::Shared(database::SharedEngine::MongoDb)),
            serde_json::to_value(self.0).unwrap(),
            serde_json::Value::Null,
        ))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputWrapper(ShuttleResourceOutput<DatabaseResource>);

#[async_trait]
impl IntoResource<String> for OutputWrapper {
    async fn into_resource(self) -> Result<String, Error> {
        Ok(match self.0.output {
            DatabaseResource::ConnectionString(s) => s.clone(),
            DatabaseResource::Info(info) => info.connection_string_shuttle(),
        })
    }
}

#[async_trait]
impl IntoResource<mongodb::Database> for OutputWrapper {
    async fn into_resource(self) -> Result<mongodb::Database, Error> {
        let connection_string: String = self.into_resource().await.unwrap();

        let mut client_options = mongodb::options::ClientOptions::parse(connection_string)
            .await
            .map_err(shuttle_service::CustomError::new)?;
        client_options.min_pool_size = Some(1);
        client_options.max_pool_size = Some(5);

        let client = mongodb::Client::with_options(client_options)
            .map_err(shuttle_service::CustomError::new)?;

        // Return a handle to the database defined at the end of the connection string,
        // which is the users provisioned database
        let database = client.default_database();

        database.ok_or_else(|| {
            Error::Database("mongodb connection string missing default database".into())
        })
    }
}
