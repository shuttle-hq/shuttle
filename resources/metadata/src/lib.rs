use async_trait::async_trait;
pub use shuttle_service::{DeploymentMetadata as Metadata, Environment, SecretStore};
use shuttle_service::{Error, ResourceFactory, ResourceInputBuilder};

#[derive(Default)]
pub struct ShuttleMetadata;

#[async_trait]
impl ResourceInputBuilder for ShuttleMetadata {
    type Input = Metadata;
    type Output = Metadata;

    async fn build(self, factory: &ResourceFactory) -> Result<Self::Input, crate::Error> {
        Ok(factory.get_metadata())
    }
}
