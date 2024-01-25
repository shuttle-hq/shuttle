use async_trait::async_trait;
use shuttle_service::{error::Error, resource::Type, Factory, ResourceBuilder};
pub use shuttle_service::{DeploymentMetadata as Metadata, Environment};

#[derive(Default)]
pub struct ShuttleMetadata;

#[async_trait]
impl ResourceBuilder for ShuttleMetadata {
    const TYPE: Type = Type::Custom;
    type Config = ();
    type Output = Metadata;

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        Ok(factory.get_metadata())
    }
}
