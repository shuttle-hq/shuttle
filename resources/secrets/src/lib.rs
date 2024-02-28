#![doc = include_str!("../README.md")]
use async_trait::async_trait;
pub use shuttle_service::SecretStore;
use shuttle_service::{
    resource::{ProvisionResourceRequest, ShuttleResourceOutput, Type},
    Error, ResourceFactory, ResourceInputBuilder,
};

/// Secrets plugin that provides service secrets
#[derive(Default)]
pub struct Secrets;

#[async_trait]
impl ResourceInputBuilder for Secrets {
    type Input = ProvisionResourceRequest;
    type Output = ShuttleResourceOutput<SecretStore>;

    async fn build(self, _factory: &ResourceFactory) -> Result<Self::Input, crate::Error> {
        Ok(ProvisionResourceRequest::new(
            Type::Secrets,
            serde_json::Value::Null,
            serde_json::Value::Null,
        ))
    }
}
