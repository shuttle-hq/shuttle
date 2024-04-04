use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database,
    resource::{ProvisionResourceRequest, ShuttleResourceOutput, Type},
    DatabaseResource, DbInput, Error, IntoResource, ResourceFactory, ResourceInputBuilder,
};

#[cfg(any(feature = "diesel-async-bb8", feature = "diesel-async-deadpool"))]
use diesel_async::pooled_connection::AsyncDieselConnectionManager;

#[cfg(feature = "diesel-async-bb8")]
use diesel_async::pooled_connection::bb8 as diesel_bb8;

#[cfg(feature = "diesel-async-deadpool")]
use diesel_async::pooled_connection::deadpool as diesel_deadpool;

#[allow(dead_code)]
const MIN_CONNECTIONS: u32 = 1;
#[allow(dead_code)]
const MAX_CONNECTIONS: u32 = 5;

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

#[cfg(feature = "diesel-async")]
#[async_trait]
impl IntoResource<diesel_async::AsyncPgConnection> for OutputWrapper {
    async fn into_resource(self) -> Result<diesel_async::AsyncPgConnection, Error> {
        use diesel_async::{AsyncConnection, AsyncPgConnection};

        let connection_string: String = self.into_resource().await.unwrap();
        Ok(AsyncPgConnection::establish(&connection_string)
            .await
            .map_err(shuttle_service::error::CustomError::new)?)
    }
}

#[cfg(feature = "diesel-async-bb8")]
#[async_trait]
impl IntoResource<diesel_bb8::Pool<diesel_async::AsyncPgConnection>> for OutputWrapper {
    async fn into_resource(
        self,
    ) -> Result<diesel_bb8::Pool<diesel_async::AsyncPgConnection>, Error> {
        let connection_string: String = self.into_resource().await.unwrap();

        Ok(diesel_bb8::Pool::builder()
            .min_idle(Some(MIN_CONNECTIONS))
            .max_size(MAX_CONNECTIONS)
            .build(AsyncDieselConnectionManager::new(connection_string))
            .await
            .map_err(shuttle_service::error::CustomError::new)?)
    }
}

#[cfg(feature = "diesel-async-deadpool")]
#[async_trait]
impl IntoResource<diesel_deadpool::Pool<diesel_async::AsyncPgConnection>> for OutputWrapper {
    async fn into_resource(
        self,
    ) -> Result<diesel_deadpool::Pool<diesel_async::AsyncPgConnection>, Error> {
        let connection_string: String = self.into_resource().await.unwrap();

        Ok(
            diesel_deadpool::Pool::builder(AsyncDieselConnectionManager::new(connection_string))
                .max_size(MAX_CONNECTIONS as usize)
                .build()
                .map_err(shuttle_service::error::CustomError::new)?,
        )
    }
}

#[cfg(feature = "sqlx")]
#[async_trait]
impl IntoResource<sqlx::PgPool> for OutputWrapper {
    async fn into_resource(self) -> Result<sqlx::PgPool, Error> {
        let connection_string: String = self.into_resource().await.unwrap();

        Ok(sqlx::postgres::PgPoolOptions::new()
            .min_connections(MIN_CONNECTIONS)
            .max_connections(MAX_CONNECTIONS)
            .connect(&connection_string)
            .await
            .map_err(shuttle_service::error::CustomError::new)?)
    }
}
