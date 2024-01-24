use async_trait::async_trait;

use crate::{database, resource};

use super::Error;

/// DAL for access resources data of projects
#[async_trait]
pub trait ResourceDal: Send {
    /// Get the resources belonging to a project
    async fn get_project_resources(
        &mut self,
        project_id: &str,
        token: &str,
    ) -> Result<Vec<resource::Response>, Error>;

    /// Get only the RDS resources that belong to a project
    async fn get_project_rds_resources(
        &mut self,
        project_id: &str,
        token: &str,
    ) -> Result<Vec<resource::Response>, Error> {
        let rds_resources = self
            .get_project_resources(project_id, token)
            .await?
            .into_iter()
            .filter(|r| {
                matches!(
                    r.r#type,
                    resource::Type::Database(database::Type::AwsRds(_))
                )
            })
            .collect();

        Ok(rds_resources)
    }
}
