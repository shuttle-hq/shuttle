#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use paste::paste;
use serde::{Deserialize, Serialize};
use shuttle_service::{
    database::{self, AwsRdsEngine},
    error::CustomError,
    Factory, ResourceBuilder, Type,
};

#[derive(Deserialize, Serialize)]
pub enum AwsRdsOutput {
    Rds(shuttle_service::DatabaseReadyInfo),
    Local(String),
}

macro_rules! aws_engine {
    ($feature:expr, $pool_path:path, $options_path:path, $struct_ident:ident) => {
        paste! {
            #[derive(Serialize)]
            #[cfg(feature = $feature)]
            #[doc = "A resource connected to an AWS RDS " $struct_ident " instance"]
            pub struct $struct_ident{
                local_uri: Option<String>,
            }

            #[cfg(feature = $feature)]
            #[doc = "Gets a `sqlx::Pool` connected to an AWS RDS " $struct_ident " instance"]
            #[async_trait]
            impl ResourceBuilder<$pool_path> for $struct_ident {
                const TYPE: Type = Type::Database(database::Type::AwsRds(AwsRdsEngine::$struct_ident));

                type Output = AwsRdsOutput;

                fn new() -> Self {
                    Self { local_uri: None }
                }

                async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, shuttle_service::Error> {
                    let info = match factory.get_environment() {
                        shuttle_service::Environment::Production => AwsRdsOutput::Rds(
                            factory
                                .get_db_connection(database::Type::AwsRds(AwsRdsEngine::$struct_ident))
                                .await?
                        ),
                        shuttle_service::Environment::Local => {
                            if let Some(local_uri) = self.local_uri {
                                AwsRdsOutput::Local(local_uri)
                            } else {
                                AwsRdsOutput::Rds(
                                    factory
                                        .get_db_connection(database::Type::AwsRds(AwsRdsEngine::$struct_ident))
                                        .await?
                                )
                            }
                        }
                    };

                    Ok(info)
                }

                async fn build(build_data: &Self::Output) -> Result<$pool_path, shuttle_service::Error> {
                    let connection_string = match build_data {
                        AwsRdsOutput::Local(local_uri) => local_uri.clone(),
                        AwsRdsOutput::Rds(info) => info.connection_string_private(),
                    };

                    let pool = $options_path::new()
                        .min_connections(1)
                        .max_connections(5)
                        .connect(&connection_string)
                        .await
                        .map_err(CustomError::new)?;

                    Ok(pool)
                }
            }

            #[cfg(feature = $feature)]
            impl $struct_ident {
                /// Use a custom connection string for local runs
                pub fn local_uri(mut self, local_uri: &str) -> Self {
                    self.local_uri = Some(local_uri.to_string());

                    self
                }
            }
        }
    };
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
