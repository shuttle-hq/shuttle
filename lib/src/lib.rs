use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type DeploymentId = Uuid;

/// Deployment metadata. This serves two purposes. Storing information
/// used for the deployment process and also providing the client with
/// information on the state of the deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMeta {
    pub id: DeploymentId,
    pub config: ProjectConfig,
    pub state: DeploymentStateMeta,
    pub url: String,
    pub build_logs: Option<String>,
    pub runtime_logs: Option<String>,
}

impl DeploymentMeta {
    pub fn new(config: &ProjectConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config: config.clone(),
            state: DeploymentStateMeta::QUEUED,
            url: Self::create_url(config),
            build_logs: None,
            runtime_logs: None,
        }
    }

    pub fn create_url(project_config: &ProjectConfig) -> String {
        format!("{}.unveil.sh", project_config.name)
    }
}

/// A label used to represent the deployment state in `DeploymentMeta`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStateMeta {
    QUEUED,
    BUILT,
    LOADED,
    DEPLOYED,
    ERROR,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectConfig {
    pub name: String,
}
