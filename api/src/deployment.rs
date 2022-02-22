use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::path::Path;
use rocket::{Data};
use rocket::response::Responder;
use uuid::Uuid;
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio;

use crate::{BuildSystem, ProjectConfig};

use service::Service;

pub type DeploymentId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentState {
    QUEUED,
    BUILDING,
    ERROR,
    INITIALIZING,
    READY,
    CANCELLED,
}

// TODO: Determine error handling strategy - error types or just use `anyhow`?
#[derive(Debug, Clone, Serialize, Deserialize, Responder)]
pub enum DeploymentError {
    #[response(status = 500)]
    Internal(String),
    #[response(status = 404)]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    id: DeploymentId,
    project_name: String,
    state: DeploymentState,
    url: String,
    build_logs: Option<String>,
    runtime_logs: Option<String>,
}

pub(crate) struct Deployment {
    info: DeploymentInfo,
    /// A user's particular implementation of the [`Service`] trait.
    service: Option<Box<dyn Service>>,
    /// This [`libloading::Library`] instance must be kept alive in order to use
    /// the `service` field without causing a segmentation fault.
    lib: Option<libloading::Library>,
}

impl Deployment {
    fn shareable(self) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(self))
    }
}

// could use `chashmap` here but it is unclear if we'll need to iterate
// over the whole thing at some point in the future.
type Deployments = HashMap<DeploymentId, Deployment>;

pub(crate) struct DeploymentSystem {
    build_system: Arc<Box<dyn BuildSystem>>,
    deployments: Arc<RwLock<Deployments>>,
}

impl DeploymentSystem {
    pub(crate) fn new(build_system: Box<dyn BuildSystem>) -> Self {
        Self {
            build_system: Arc::new(build_system),
            deployments: Default::default(),
        }
    }

    /// Get's the deployment information back to the user
    pub(crate) fn get_deployment(&self, id: &DeploymentId) -> Result<DeploymentInfo, DeploymentError> {
        self.deployments
            .read()
            .unwrap()
            .get(&id)
            .map(|deployment| deployment.info.clone())
            .ok_or(DeploymentError::NotFound("could not find deployment".to_string()))
    }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) fn deploy(&self,
                         crate_file: Data,
                         project_config: &ProjectConfig) -> Result<DeploymentInfo, DeploymentError> {

        // for crate file consider placing somewhere in the file system via the build system

        let info = DeploymentInfo {
            id: Uuid::new_v4(),
            project_name: project_config.name.clone(),
            state: DeploymentState::QUEUED,
            url: Self::create_url(project_config),
            build_logs: None,
            runtime_logs: None,
        };

        let deployment = Deployment {
            info,
            service: None,
            lib: None,
        };

        let info = deployment.info.clone();

        self.deployments
            .write()
            .unwrap()
            .insert(info.id.clone(), deployment);

        let build_system = self.build_system.clone();
        let deployments = self.deployments.clone();

        tokio::spawn(async move {
            Self::start_deployment_job(
                build_system,
                info.id.clone(),
                deployments,
                (),
            )
        });

        Ok(info)
    }

    async fn start_deployment_job(
        build_system: Arc<Box<dyn BuildSystem>>,
        id: DeploymentId,
        deployment: Arc<RwLock<Deployments>>,
        crate_file: ()) {
        println!("started deployment job");
        unimplemented!()
    }

    fn create_url(project_config: &ProjectConfig) -> String {
        format!("{}.unveil.sh", project_config.name)
    }
}

const ENTRYPOINT_SYMBOL_NAME: &'static [u8] = b"_create_service\0";

type CreateService = unsafe extern fn() -> *mut dyn Service;

/// Dynamically load from a `.so` file a value of a type implementing the
/// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
/// function called [`ENTRYPOINT_SYMBOL_NAME`], likely automatically generated
/// using the [`service::declare_service`] macro.
fn load_service_from_so_file(so_path: &Path) -> anyhow::Result<(Box<dyn Service>, libloading::Library)> {
    unsafe {
        let lib = libloading::Library::new(so_path)?;

        let entrypoint: libloading::Symbol<CreateService> = lib.get(ENTRYPOINT_SYMBOL_NAME)?;
        let raw = entrypoint();

        Ok((Box::from_raw(raw), lib))
    }
}
