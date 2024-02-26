#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database,
    resource::{ProvisionResourceRequest, ShuttleResourceOutput, Type},
    DatabaseResource, DbInput, Error, IntoResource, ResourceFactory, ResourceInputBuilder,
};

macro_rules! aws_engine {
    ($feature:expr, $struct_ident:ident) => {
        paste::paste! {
            #[cfg(feature = $feature)]
            #[derive(Default)]
            #[doc = "Shuttle managed AWS RDS " $struct_ident " instance"]
            pub struct $struct_ident(DbInput);

            #[cfg(feature = $feature)]
            impl $struct_ident {
                /// Use a custom connection string for local runs
                pub fn local_uri(mut self, local_uri: &str) -> Self {
                    self.0.local_uri = Some(local_uri.to_string());

                    self
                }
            }

            #[cfg(feature = $feature)]
            #[async_trait::async_trait]
            impl ResourceInputBuilder for $struct_ident {
                type Input = ProvisionResourceRequest;
                type Output = OutputWrapper;

                async fn build(self, _factory: &ResourceFactory) -> Result<Self::Input, Error> {
                    Ok(ProvisionResourceRequest::new(
                        Type::Database(
                            database::Type::AwsRds(
                                database::AwsRdsEngine::$struct_ident
                            )
                        ),
                        serde_json::to_value(&self.0).unwrap(),
                        serde_json::Value::Null,
                    ))
                }
            }
        }
    };
}

aws_engine!("postgres", Postgres);

aws_engine!("mysql", MySql);

aws_engine!("mariadb", MariaDB);

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

// If these were done in the main macro above, this would produce two conflicting `impl IntoResource<sqlx::MySqlPool>`
#[cfg(feature = "sqlx")]
mod _sqlx {
    use super::*;

    #[cfg(feature = "postgres")]
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

    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    #[async_trait]
    impl IntoResource<sqlx::MySqlPool> for OutputWrapper {
        async fn into_resource(self) -> Result<sqlx::MySqlPool, Error> {
            let connection_string: String = self.into_resource().await.unwrap();

            Ok(sqlx::mysql::MySqlPoolOptions::new()
                .min_connections(1)
                .max_connections(5)
                .connect(&connection_string)
                .await
                .map_err(shuttle_service::error::CustomError::new)?)
        }
    }
}
