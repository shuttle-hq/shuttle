use async_trait::async_trait;
pub use shuttle_service::{DeploymentMetadata as Metadata, Environment, SecretStore};
use shuttle_service::{Error, ResourceFactory, ResourceInputBuilder};

#[derive(Default)]
#[deprecated(
    since = "0.42.0",
    note = "This plugin has been moved to shuttle_runtime::Metadata, see https://docs.shuttle.rs/resources/shuttle-metadata"
)]
pub struct ShuttleMetadata;

#[async_trait]
impl ResourceInputBuilder for ShuttleMetadata {
    type Input = Metadata;
    type Output = Metadata;

    async fn build(self, factory: &ResourceFactory) -> Result<Self::Input, crate::Error> {
        Ok(factory.get_metadata())
    }
}
