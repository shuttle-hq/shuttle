#![doc = include_str!("../README.md")]
use async_trait::async_trait;
pub use shuttle_service::SecretStore;
use shuttle_service::{
    resource::{ProvisionResourceRequest, ShuttleResourceOutput, Type},
    Error, Factory, IntoResourceInput,
};

/// Secrets plugin that provides service secrets
#[derive(Default)]
pub struct Secrets;

#[async_trait]
impl IntoResourceInput for Secrets {
    type Input = ProvisionResourceRequest;
    type Output = ShuttleResourceOutput<SecretStore>;

    async fn into_resource_input(self, _factory: &dyn Factory) -> Result<Self::Input, crate::Error> {
        Ok(ProvisionResourceRequest::new(
            Type::Secrets,
            serde_json::Value::Null,
            serde_json::Value::Null,
        ))
    }
}
