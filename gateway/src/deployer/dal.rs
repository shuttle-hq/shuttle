use shuttle_common::{
    models::project::ProjectName,
    persistence::{deployment::Deployment, service::Service, state::State},
};
use sqlx::{Row, SqlitePool};
use ulid::Ulid;

use super::error::{Error, Result};

pub trait Dal {
    async fn project_name_by_admin_secret(&self, secret: &str) -> Result<Option<ProjectName>>;
    async fn get_service_by_name(&self, name: &str) -> Result<Option<Service>>;
    async fn get_active_deployment(&self, service_id: &Ulid) -> Result<Option<Deployment>>;
}

impl Dal for SqlitePool {
    async fn project_name_by_admin_secret(&self, secret: &str) -> Result<Option<ProjectName>> {
        sqlx::query("SELECT project_name FROM projects WHERE initial_key = ?")
            .bind(secret)
            .fetch_optional(self)
            .await
            .map(|o| {
                o.map(|r| {
                    r.try_get::<ProjectName, _>("project_name")
                        .expect("to have a value")
                })
            })
            .map_err(Error::from)
    }

    async fn get_service_by_name(&self, name: &str) -> Result<Option<Service>> {
        sqlx::query_as("SELECT * FROM services WHERE name = ?")
            .bind(name)
            .fetch_optional(self)
            .await
            .map_err(Error::from)
    }

    async fn get_active_deployment(&self, service_id: &Ulid) -> Result<Option<Deployment>> {
        sqlx::query_as("SELECT * FROM deployments WHERE service_id = ? AND state = ?")
            .bind(service_id.to_string())
            .bind(State::Running)
            .fetch_optional(self)
            .await
            .map_err(Error::from)
    }
}
