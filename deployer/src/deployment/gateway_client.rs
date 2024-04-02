use shuttle_backends::client::{Error, ServicesApiClient};
use shuttle_common::models;
use uuid::Uuid;

/// A client that can communicate with the build queue
#[async_trait::async_trait]
pub trait BuildQueueClient: Clone + Send + Sync + 'static {
    /// Try to get a build slot. A false returned value means that the spot could not be acquire
    async fn get_slot(&self, id: Uuid) -> Result<bool, Error>;

    /// Release a build slot that was previously acquired
    async fn release_slot(&self, id: Uuid) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl BuildQueueClient for ServicesApiClient {
    async fn get_slot(&self, deployment_id: Uuid) -> Result<bool, Error> {
        let load: models::stats::LoadResponse = self
            .post(
                "stats/load",
                models::stats::LoadRequest { id: deployment_id },
                None,
            )
            .await?;

        Ok(load.has_capacity)
    }

    async fn release_slot(&self, deployment_id: Uuid) -> Result<(), Error> {
        let _load: models::stats::LoadResponse = self
            .delete(
                "stats/load",
                models::stats::LoadRequest { id: deployment_id },
                None,
            )
            .await?;

        Ok(())
    }
}
