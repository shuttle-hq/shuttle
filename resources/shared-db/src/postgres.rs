use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{
    database, error::CustomError, DbOutput, Error, Factory, ResourceBuilder, Type,
};

#[derive(Serialize)]
pub struct Postgres {
    local_uri: Option<String>,
}

/// Get an `sqlx::PgPool` from any factory
#[async_trait]
impl ResourceBuilder<sqlx::PgPool> for Postgres {
    const TYPE: Type = Type::Database(database::Type::Shared(database::SharedEngine::Postgres));

    type Output = DbOutput;

    fn new() -> Self {
        Self { local_uri: None }
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        let info = match factory.get_environment() {
            shuttle_service::Environment::Production => DbOutput::Info(
                factory
                    .get_db_connection(database::Type::Shared(database::SharedEngine::Postgres))
                    .await?,
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.local_uri {
                    DbOutput::Local(local_uri)
                } else {
                    DbOutput::Info(
                        factory
                            .get_db_connection(database::Type::Shared(
                                database::SharedEngine::Postgres,
                            ))
                            .await?,
                    )
                }
            }
        };

        Ok(info)
    }

    async fn build(build_data: &Self::Output) -> Result<sqlx::PgPool, Error> {
        let connection_string = match build_data {
            DbOutput::Local(local_uri) => local_uri.clone(),
            DbOutput::Info(info) => info.connection_string_private(),
        };

        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect(&connection_string)
            .await
            .map_err(CustomError::new)?;

        Ok(pool)
    }
}

impl Postgres {
    /// Use a custom connection string for local runs
    pub fn local_uri(mut self, local_uri: &str) -> Self {
        self.local_uri = Some(local_uri.to_string());

        self
    }
}
