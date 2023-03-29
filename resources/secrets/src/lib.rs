use async_trait::async_trait;

use serde::Serialize;
pub use shuttle_service::SecretStore;
use shuttle_service::{Error, Factory, ResourceBuilder, Type};

#[derive(Serialize)]
pub struct Secrets;

/// Get a store with all the secrets available to a deployment
#[async_trait]
impl ResourceBuilder<SecretStore> for Secrets {
    const TYPE: Type = Type::Secrets;

    type Output = SecretStore;

    fn new() -> Self {
        Self {}
    }

    async fn build(build_data: &Self::Output) -> Result<SecretStore, crate::Error> {
        Ok(build_data.clone())
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, crate::Error> {
        let secrets = factory.get_secrets().await?;

        Ok(SecretStore::new(secrets))
    }
}
