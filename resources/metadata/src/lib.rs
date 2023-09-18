use async_trait::async_trait;
use shuttle_service::{error::Error, DeploymentMetadata, Factory, ResourceBuilder, Type};

pub struct ShuttleMetadata;

#[async_trait]
impl ResourceBuilder<DeploymentMetadata> for ShuttleMetadata {
    fn new() -> Self {
        Self
    }

    const TYPE: Type = Type::Metadata;

    type Config = ();

    type Output = DeploymentMetadata;

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        Ok(factory.get_metadata())
    }

    async fn build(build_data: &Self::Output) -> Result<DeploymentMetadata, Error> {
        Ok(build_data.clone())
    }
}
