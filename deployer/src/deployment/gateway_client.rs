use axum::headers::{authorization::Bearer, Authorization};
use hyper::Method;
use shuttle_common::{
    backends::client::{gateway, Error},
    models::{self},
};
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
impl BuildQueueClient for gateway::Client {
    async fn get_slot(&self, deployment_id: Uuid) -> Result<bool, Error> {
        let body = models::stats::LoadRequest { id: deployment_id };
        let load: models::stats::LoadResponse = self
            .public_client()
            .request(
                Method::POST,
                "stats/load",
                Some(body),
                None::<Authorization<Bearer>>,
            )
            .await?;

        Ok(load.has_capacity)
    }

    async fn release_slot(&self, deployment_id: Uuid) -> Result<(), Error> {
        let body = models::stats::LoadRequest { id: deployment_id };
        let _load: models::stats::LoadResponse = self
            .public_client()
            .request(
                Method::DELETE,
                "stats/load",
                Some(body),
                None::<Authorization<Bearer>>,
            )
            .await?;

        Ok(())
    }
}
