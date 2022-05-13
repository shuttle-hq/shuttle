pub enum Type {
    AwsRdsPostgres,
    Shared,
}

#[cfg(feature = "sqlx-aws")]
pub mod aws_rds {
    use super::Type;
    use crate::{error::CustomError, Factory, GetResource};
    use async_trait::async_trait;
    use sqlx::PgPool;
    use tokio::runtime::Runtime;

    pub struct Postgres(pub PgPool);

    /// Get an `sqlx::PgPool` connected to an AWS RDS Postgres instance
    #[async_trait]
    impl GetResource<Postgres> for &mut dyn Factory {
        async fn get_resource(self, runtime: &Runtime) -> Result<Postgres, crate::Error> {
            let connection_string = self.get_sql_connection_string(Type::AwsRdsPostgres).await?;

            // A sqlx Pool cannot cross runtime boundaries, so make sure to create the Pool on the service end
            let pool = runtime
                .spawn(async move {
                    sqlx::postgres::PgPoolOptions::new()
                        .min_connections(1)
                        .max_connections(5)
                        .connect(&connection_string)
                        .await
                })
                .await
                .map_err(CustomError::new)?
                .map_err(CustomError::new)?;

            Ok(Postgres(pool))
        }
    }
}
