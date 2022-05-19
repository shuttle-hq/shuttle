use std::fmt::Display;

pub enum Type {
    AwsRds(AwsRdsEngine),
    Shared,
}

pub enum AwsRdsEngine {
    Postgres,
    MySql,
    MariaDB,
}

impl Display for AwsRdsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MariaDB => write!(f, "mariadb"),
            Self::MySql => write!(f, "mysql"),
            Self::Postgres => write!(f, "postgres"),
        }
    }
}

macro_rules! aws_engine {
    ($feature:expr, $pool_path:path, $options_path:path, $struct_ident:ident) => {
        paste! {
            #[cfg(feature = $feature)]
            #[doc = "A resource connected to an AWS RDS " $struct_ident " instance"]
            pub struct $struct_ident(pub $pool_path);

            #[cfg(feature = $feature)]
            #[doc = "Gets a `sqlx::Pool` connected to an AWS RDS " $struct_ident " instance"]
            #[async_trait]
            impl GetResource<$struct_ident> for &mut dyn Factory {
                async fn get_resource(self, runtime: &Runtime) -> Result<$struct_ident, crate::Error> {
                    let connection_string = self
                        .get_sql_connection_string(Type::AwsRds(AwsRdsEngine::$struct_ident))
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

                    Ok($struct_ident(pool))
                }
            }
        }
    };
}

pub mod aws_rds {
    use super::{AwsRdsEngine, Type};
    use crate::{error::CustomError, Factory, GetResource};
    use async_trait::async_trait;
    use paste::paste;
    use tokio::runtime::Runtime;

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
}
