#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{DatabaseResource, Error};

macro_rules! aws_engine {
    ($feature:expr, $pool_path:path, $options_path:path, $struct_ident:ident) => {
        paste::paste! {
            #[cfg(feature = $feature)]
            #[derive(Default)]
            #[doc = "Shuttle managed AWS RDS " $struct_ident " instance"]
            pub struct $struct_ident(shuttle_service::DbInput);

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
            impl shuttle_service::ResourceBuilder for $struct_ident {
                const TYPE: shuttle_service::Type = shuttle_service::Type::Database(
                    shuttle_service::database::Type::AwsRds(
                        shuttle_service::database::AwsRdsEngine::$struct_ident
                    )
                );

                type Config = shuttle_service::DbInput;
                type Output = Wrap;

                fn config(&self) -> &Self::Config {
                    &self.0
                }

                async fn output(self, factory: &mut dyn shuttle_service::Factory) -> Result<Self::Output, shuttle_service::Error> {
                    let info = match factory.get_metadata().env {
                        shuttle_service::Environment::Deployment => shuttle_service::DatabaseResource::Info(
                            factory
                                .get_db_connection(shuttle_service::database::Type::AwsRds(shuttle_service::database::AwsRdsEngine::$struct_ident))
                                .await?
                        ),
                        shuttle_service::Environment::Local => {
                            if let Some(local_uri) = self.0.local_uri {
                                shuttle_service::DatabaseResource::ConnectionString(local_uri)
                            } else {
                                shuttle_service::DatabaseResource::Info(
                                    factory
                                        .get_db_connection(shuttle_service::database::Type::AwsRds(shuttle_service::database::AwsRdsEngine::$struct_ident))
                                        .await?
                                )
                            }
                        }
                    };

                    Ok(Wrap(info))
                }
            }

            #[cfg(all(feature = "sqlx", feature = $feature))]
            #[async_trait]
            impl IntoResource<$pool_path> for Wrap {
                async fn into_resource(self) -> Result<$pool_path, Error> {
                    let connection_string: String = self.into_resource().await.unwrap();

                    Ok($options_path::new()
                        .min_connections(1)
                        .max_connections(5)
                        .connect(&connection_string)
                        .await
                        .map_err(shuttle_service::error::CustomError::new)?)
                }
            }
        }
    };
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

aws_engine!(
    "postgres",
    sqlx::PgPool,
    sqlx::postgres::PgPoolOptions,
    Postgres
);

aws_engine!(
    "mysql",
    sqlx::MySqlPool,
    sqlx::mysql::MySqlPoolOptions,
    MySql
);

aws_engine!(
    "mariadb",
    sqlx::MySqlPool,
    sqlx::mysql::MySqlPoolOptions,
    MariaDB
);
