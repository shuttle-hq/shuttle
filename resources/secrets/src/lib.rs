use std::collections::BTreeMap;

use async_trait::async_trait;

use shuttle_service::{Error, Factory, ResourceBuilder};

pub struct Secrets;

/// Get a store with all the secrets available to a deployment
#[async_trait]
impl ResourceBuilder<SecretStore> for Secrets {
    fn new() -> Self {
        Self {}
    }

    async fn build(self, factory: &mut dyn Factory) -> Result<SecretStore, Error> {
        let secrets = factory.get_secrets().await?;

        Ok(SecretStore { secrets })
    }
}

/// Store that holds all the secrets available to a deployment
pub struct SecretStore {
    secrets: BTreeMap<String, String>,
}

impl SecretStore {
    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(ToOwned::to_owned)
    }
}
