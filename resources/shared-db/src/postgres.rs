use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database, resource::{Type, Response}, DatabaseResource, DbInput, Error, Factory, IntoResource,
    IntoResourceInput,
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
impl IntoResourceInput for Postgres {
    type Input = Response;
    type Output = Wrapper;

    async fn into_resource_input(self, _factory: &dyn Factory) -> Result<Self::Input, Error> {
        Ok(Response {
            r#type: database::Type::Shared(database::SharedEngine::Postgres),
            config: self.0,
            data: Default::default(), // null
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Wrapper(DatabaseResource);

#[async_trait]
impl IntoResource<String> for Wrapper {
    async fn into_resource(self) -> Result<String, Error> {
        Ok(match self.0 {
            DatabaseResource::ConnectionString(s) => s.clone(),
            DatabaseResource::Info(info) => info.connection_string_shuttle(),
        })
    }
}

#[cfg(feature = "sqlx")]
#[async_trait]
impl IntoResource<sqlx::PgPool> for Wrapper {
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
