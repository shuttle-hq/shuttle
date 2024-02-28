use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database,
    resource::{ProvisionResourceRequest, ShuttleResourceOutput, Type},
    DatabaseResource, DbInput, Error, IntoResource, ResourceFactory, ResourceInputBuilder,
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
impl ResourceInputBuilder for Postgres {
    type Input = ProvisionResourceRequest;
    type Output = OutputWrapper;

    async fn build(self, _factory: &ResourceFactory) -> Result<Self::Input, Error> {
        Ok(ProvisionResourceRequest::new(
            Type::Database(database::Type::Shared(database::SharedEngine::Postgres)),
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

#[cfg(feature = "sqlx")]
#[async_trait]
impl IntoResource<sqlx::PgPool> for OutputWrapper {
    async fn into_resource(self) -> Result<sqlx::PgPool, Error> {
        let connection_string: String = self.into_resource().await.unwrap();

        Ok(sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect(&connection_string)
            .await
            .map_err(shuttle_service::error::CustomError::new)?)
    }
}
