use crate::{
    database::{AwsRdsEngine, Type},
    error::CustomError,
    Factory, ResourceBuilder,
};
use async_trait::async_trait;
use paste::paste;
use tokio::runtime::Runtime;

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

                async fn build(self, factory: &mut dyn Factory, runtime: &Runtime) -> Result<$pool_path, crate::Error> {
                    let connection_string = factory
                        .get_db_connection_string(Type::AwsRds(AwsRdsEngine::$struct_ident))
                        .await?;

                    // A sqlx Pool cannot cross runtime boundaries, so make sure to create the Pool on the service end
                    let pool = runtime
                        .spawn(async move {
                            $options_path::new()
                                .min_connections(1)
                                .max_connections(5)
                                .connect(&connection_string)
                                .await
                        })
                        .await
                        .map_err(CustomError::new)?
                        .map_err(CustomError::new)?;

                    Ok(pool)
                }
            }
        }
    };
}

aws_engine!(
    "sqlx-aws-postgres",
    sqlx::PgPool,
    sqlx::postgres::PgPoolOptions,
    Postgres
);

aws_engine!(
    "sqlx-aws-mysql",
    sqlx::MySqlPool,
    sqlx::mysql::MySqlPoolOptions,
    MySql
);

aws_engine!(
    "sqlx-aws-mariadb",
    sqlx::MySqlPool,
    sqlx::mysql::MySqlPoolOptions,
    MariaDB
);
