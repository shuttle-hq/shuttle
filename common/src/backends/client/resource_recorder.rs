use crate::{claims::Claim, database, resource};

use super::Error;

#[allow(async_fn_in_trait)]
pub trait ResourceDal {
    async fn get_project_resources(
        &mut self,
        project_id: &str,
        claim: &Claim,
    ) -> Result<impl Iterator<Item = resource::Response>, Error>;

    async fn get_project_rds_resources(
        &mut self,
        project_id: &str,
        claim: &Claim,
    ) -> Result<impl Iterator<Item = resource::Response>, Error> {
        let rds_resources = self
            .get_project_resources(project_id, claim)
            .await?
            .into_iter()
            .filter(|r| {
                matches!(
                    r.r#type,
                    resource::Type::Database(database::Type::AwsRds(_))
                )
            });

        Ok(rds_resources)
    }
}
