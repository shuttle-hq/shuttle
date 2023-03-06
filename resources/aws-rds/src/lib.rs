#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use paste::paste;
use shuttle_service::{
    database::{AwsRdsEngine, Type},
    error::CustomError,
    Factory, ResourceBuilder,
};

macro_rules! aws_engine {
    ($feature:expr, $pool_path:path, $options_path:path, $struct_ident:ident) => {
        paste! {
            #[cfg(feature = $feature)]
            #[doc = "A resource connected to an AWS RDS " $struct_ident " instance"]
            pub struct $struct_ident;

            #[cfg(feature = $feature)]
            #[doc = "Gets a `sqlx::Pool` connected to an AWS RDS " $struct_ident " instance"]
            #[async_trait]
            impl ResourceBuilder<$pool_path> for $struct_ident {
                fn new() -> Self {
                    Self {}
                }

                async fn build(self, factory: &mut dyn Factory) -> Result<$pool_path, shuttle_service::Error> {
                    let connection_string = factory
                        .get_db_connection_string(Type::AwsRds(AwsRdsEngine::$struct_ident))
                        .await?;

                    let pool = $options_path::new()
                        .min_connections(1)
                        .max_connections(5)
                        .connect(&connection_string)
                        .await
                        .map_err(CustomError::new)?;

                    Ok(pool)
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
