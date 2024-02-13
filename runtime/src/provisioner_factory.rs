use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;
use shuttle_common::{constants::STORAGE_DIRNAME, secrets::Secret};
use shuttle_service::{DeploymentMetadata, Environment, Factory};

/// A factory (service locator) which goes through the provisioner crate
pub struct ProvisionerFactory {
    pub(crate) project_name: String,
    pub(crate) secrets: BTreeMap<String, Secret<String>>,
    pub(crate) env: Environment,
}

#[async_trait]
impl Factory for ProvisionerFactory {
    fn get_secrets(&self) -> Result<BTreeMap<String, Secret<String>>, shuttle_service::Error> {
        Ok(self.secrets.clone())
    }

    fn get_metadata(&self) -> DeploymentMetadata {
        DeploymentMetadata {
            env: self.env,
            project_name: self.project_name.to_string(),
            storage_path: PathBuf::from(STORAGE_DIRNAME),
        }
    }
}
