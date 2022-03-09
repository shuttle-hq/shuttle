use crate::database;
use async_trait::async_trait;
use lib::project::ProjectConfig;
use sqlx::{postgres::PgPoolOptions, PgPool};
use unveil_service::Factory;

pub(crate) struct UnveilFactory<'a> {
    database: &'a mut database::State,
    project: ProjectConfig,
    ctx: database::Context,
}

impl<'a> UnveilFactory<'a> {
    pub(crate) fn new(
        database: &'a mut database::State,
        project: ProjectConfig,
        ctx: database::Context,
    ) -> Self {
        Self {
            database,
            project,
            ctx,
        }
    }
}

#[async_trait]
impl Factory for UnveilFactory<'_> {
    async fn get_sql_connection_string(&mut self) -> Result<String, unveil_service::Error> {
        let ready_state = self
            .database
            .advance(&self.project.name(), &self.ctx)
            .await
            .map_err(unveil_service::Error::from)?;

        Ok(ready_state.connection_string("localhost"))

    }
    /// Lazily gets a connection pool
    async fn get_postgres_connection_pool(&mut self) -> Result<PgPool, unveil_service::Error> {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(&self.get_sql_connection_string().await?)
            .await
            .map_err(unveil_service::Error::from)
    }
}
