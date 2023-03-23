use std::collections::BTreeMap;

use async_trait::async_trait;

use serde::{Deserialize, Serialize};
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

        Ok(SecretStore { secrets })
    }
}

/// Store that holds all the secrets available to a deployment
#[derive(Deserialize, Serialize, Clone)]
pub struct SecretStore {
    secrets: BTreeMap<String, String>,
}

impl SecretStore {
    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(ToOwned::to_owned)
    }
}
