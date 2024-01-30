use shuttle_common::models::project::ProjectName;
use sqlx::{Row, SqlitePool};

use super::error::{Error, Result};

pub trait Dal {
    async fn project_name_by_admin_secret(&self, secret: &str) -> Result<Option<ProjectName>>;
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
}
