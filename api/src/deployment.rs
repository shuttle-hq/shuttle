use std::collections::HashMap;
use std::sync::{Arc};
use std::time::Duration;
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
pub struct DeploymentInfo {
    id: DeploymentId,
    project_name: String,
    state: DeploymentState,
    url: String,
    build_logs: Option<String>,
    runtime_logs: Option<String>
}

pub(crate) trait Service: Send + Sync {

}

impl Service for () {

}

pub(crate) type Library = ();

pub(crate) struct Deployment {
    info: DeploymentInfo,
    service: Box<dyn Service>,
    so: Library
}

type Deployments = HashMap<DeploymentId, Deployment>;

pub(crate) struct DeploymentSystem {
    build_system: Box<dyn BuildSystem>,
    deployments: Deployments,
}

impl DeploymentSystem {
    pub(crate) fn new(build_system: Box<dyn BuildSystem>) -> Self {
        Self {
            build_system,
            deployments: Default::default()
        }
    }

    /// Get's the deployment information back to the user
    pub(crate) fn get_deployment(&self, id: &DeploymentId) -> Option<DeploymentInfo> {
        self.deployments.get(id).map(|d| d.info.clone())
    }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) fn deploy(&mut self,
                               crate_file: Data,
                               project_config: &ProjectConfig) -> Result<DeploymentInfo, DeploymentError> {

        let info = DeploymentInfo {
            id: Uuid::new_v4(),
            project_name: project_config.name.clone(),
            state: DeploymentState::QUEUED,
            url: Self::create_url(project_config),
            build_logs: None,
            runtime_logs: None
        };

        let deployment = Deployment {
            info,
            service: Box::new(()),
            so: ()
        };

        let info = deployment.info.clone();

        self.deployments.insert(deployment.info.id.clone(), deployment);

        Ok(info)
    }

    fn create_url(project_config: &ProjectConfig) -> String {
        format!("{}.unveil.sh", project_config.name)
    }
}