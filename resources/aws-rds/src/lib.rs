#![doc = include_str!("../README.md")]

macro_rules! aws_engine {
    ($feature:expr, $pool_path:path, $options_path:path, $struct_ident:ident) => {
        paste::paste! {
            #[derive(serde::Serialize)]
            #[cfg(feature = $feature)]
            #[doc = "A resource connected to an AWS RDS " $struct_ident " instance"]
            pub struct $struct_ident{
                config: shuttle_service::DbInput,
            }

            #[cfg(feature = $feature)]
            #[doc = "Gets a `sqlx::Pool` connected to an AWS RDS " $struct_ident " instance"]
            #[async_trait::async_trait]
            impl shuttle_service::ResourceBuilder<$pool_path> for $struct_ident {
                const TYPE: shuttle_service::Type = shuttle_service::Type::Database(
                    shuttle_service::database::Type::AwsRds(
                        shuttle_service::database::AwsRdsEngine::$struct_ident
                    )
                );

                type Config = shuttle_service::DbInput;
                type Output = shuttle_service::DbOutput;

                fn new() -> Self {
                    Self { config: Default::default() }
                }

                fn config(&self) -> &Self::Config {
                    &self.config
                }

                async fn output(self, factory: &mut dyn shuttle_service::Factory) -> Result<Self::Output, shuttle_service::Error> {
                    let info = match factory.get_metadata().env {
                        shuttle_service::Environment::Production => shuttle_service::DbOutput::Info(
                            factory
                                .get_db_connection(shuttle_service::database::Type::AwsRds(shuttle_service::database::AwsRdsEngine::$struct_ident))
                                .await?
                        ),
                        shuttle_service::Environment::Local => {
                            if let Some(local_uri) = self.config.local_uri {
                                shuttle_service::DbOutput::Local(local_uri)
                            } else {
                                shuttle_service::DbOutput::Info(
                                    factory
                                        .get_db_connection(shuttle_service::database::Type::AwsRds(shuttle_service::database::AwsRdsEngine::$struct_ident))
                                        .await?
                                )
                            }
                        }
                    };

                    Ok(info)
                }

                async fn build(build_data: &Self::Output) -> Result<$pool_path, shuttle_service::Error> {
                    let connection_string = match build_data {
                        shuttle_service::DbOutput::Local(local_uri) => local_uri.clone(),
                        shuttle_service::DbOutput::Info(info) => info.connection_string_private(),
                    };

                    let pool = $options_path::new()
                        .min_connections(1)
                        .max_connections(5)
                        .connect(&connection_string)
                        .await
                        .map_err(shuttle_service::error::CustomError::new)?;

                    Ok(pool)
                }
            }

            #[cfg(feature = $feature)]
            impl $struct_ident {
                /// Use a custom connection string for local runs
                pub fn local_uri(mut self, local_uri: &str) -> Self {
                    self.config.local_uri = Some(local_uri.to_string());

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
