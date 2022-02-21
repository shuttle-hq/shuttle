use std::collections::HashMap;
use std::sync::{Arc};
use rocket::{Data, Request};
use rocket::response::Responder;
use uuid::Uuid;
use rocket::serde::{Serialize, Deserialize};

use crate::{BuildSystem, ProjectConfig};

pub type DeploymentId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentState {
    QUEUED,
    BUILDING,
    ERROR,
    INITIALIZING,
    READY,
    CANCELLED
}

#[derive(Debug, Clone, Serialize, Deserialize, Responder)]
pub enum DeploymentError {
    #[response(status = 500)]
    Internal(String),
    #[response(status = 404)]
    NotFound(String)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    id: DeploymentId,
    project_name: String,
    state: DeploymentState,
    url: String,
    build_logs: Option<String>,
    runtime_logs: Option<String>
}

type Deployments = HashMap<DeploymentId, Deployment>;

pub(crate) struct DeploymentManager {
    build_system: Box<dyn BuildSystem>,
    deployments: Deployments,
}

impl DeploymentManager {
    pub(crate) fn new(build_system: Box<dyn BuildSystem>) -> Self {
        Self {
            build_system,
            deployments: Default::default()
        }
    }

    pub(crate) fn get_deployment(&self, id: &DeploymentId) -> Option<Deployment> {
        self.deployments.get(id).map(|d| d.clone())
    }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) fn deploy(&mut self,
                               crate_file: Data,
                               project_config: &ProjectConfig) -> Result<Deployment, DeploymentError> {

        let deployment = Deployment {
            id: Uuid::new_v4(),
            project_name: project_config.name.clone(),
            state: DeploymentState::QUEUED,
            url: Self::create_url(project_config),
            build_logs: None,
            runtime_logs: None
        };

        self.deployments.insert(deployment.id.clone(), deployment.clone());

        Ok(deployment)
    }

    fn create_url(project_config: &ProjectConfig) -> String {
        format!("{}.unveil.sh", project_config.name)
    }
}