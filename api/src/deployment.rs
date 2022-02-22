use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;
use std::path::Path;
use rocket::{Data};
use rocket::response::Responder;
use uuid::Uuid;
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio;

use crate::{BuildSystem, ProjectConfig};
use crate::build::Build;

use service::Service;

pub type DeploymentId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentState {
    QUEUED,
    BUILDING,
    LOADING,
    DEPLOYING,
    READY,
    CANCELLED,
    ERROR,
}

// TODO: Determine error handling strategy - error types or just use `anyhow`?
#[derive(Debug, Clone, Serialize, Deserialize, Responder)]
pub enum DeploymentError {
    #[response(status = 500)]
    Internal(String),
    #[response(status = 404)]
    NotFound(String),
    #[response(status = 400)]
    BadRequest(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    id: DeploymentId,
    config: ProjectConfig,
    state: DeploymentState,
    url: String,
    build_logs: Option<String>,
    runtime_logs: Option<String>,
}

pub(crate) struct Deployment {
    info: DeploymentInfo,
    /// A user's particular implementation of the [`Service`] trait.
    service: Option<Box<dyn Service>>,
    so: Option<Library>,
    build: Option<Build>,
}

pub(crate) struct Deployment {
    inner: RwLock<DeploymentInner>,
}

impl Deployment {
    pub(crate) fn info(&self) -> DeploymentInfo {
        self.inner().info.clone()
    }

    pub(crate) fn state(&self) -> DeploymentState {
        self.inner().info.state.clone()
    }

    pub(crate) fn update_state(&self, state: DeploymentState) {
        let mut inner = self.inner_mut();
        inner.info.state = state;
    }

    pub(crate) fn project_config(&self) -> ProjectConfig {
        self.inner().info.config.clone()
    }

    pub(crate) fn attach_build_artifact(&self, build: Build) {
        let mut inner = self.inner_mut();
        inner.build = Some(build)
    }

    fn inner_mut(&self) -> RwLockWriteGuard<'_, DeploymentInner> {
        self.inner.write().unwrap()
    }

    fn inner(&self) -> RwLockReadGuard<'_, DeploymentInner> {
        self.inner.read().unwrap()
    }
}

type Deployments = HashMap<DeploymentId, Arc<Deployment>>;

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
            .map(|deployment| deployment.info())
            .ok_or(DeploymentError::NotFound("could not find deployment".to_string()))
    }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) async fn deploy(&self,
                               crate_file: Data<'_>,
                               project_config: &ProjectConfig) -> Result<DeploymentInfo, DeploymentError> {

        // for crate file consider placing somewhere in the file system via the build system

        let info = DeploymentInfo {
            id: Uuid::new_v4(),
            config: project_config.clone(),
            state: DeploymentState::QUEUED,
            url: Self::create_url(project_config),
            build_logs: None,
            runtime_logs: None,
        };

        let id = info.id.clone();

        let deployment = Deployment {
            inner: RwLock::new(DeploymentInner {
                info: info.clone(),
                service: None,
                so: None,
                build: None,
            })
        };

        let deployment = Arc::new(deployment);

        self.deployments
            .write()
            .unwrap()
            .insert(id.clone(), deployment.clone());

        let build_system = self.build_system.clone();
        let deployments = self.deployments.clone();
        let crate_bytes = crate_file
            .open(ByteUnit::max_value()).into_bytes()
            .await
            .map_err(|_| DeploymentError::BadRequest("could not read crate file into bytes".to_string()))?
            .to_vec();

        tokio::spawn(async move {
            Self::start_deployment_job(
                build_system,
                id.clone(),
                deployment,
                crate_bytes,
            )
        });

        Ok(info)
    }

    async fn start_deployment_job(
        build_system: Arc<Box<dyn BuildSystem>>,
        id: DeploymentId,
        deployment: Arc<Deployment>,
        crate_file: Vec<u8>) {
        dbg!("started deployment job for id: {}", id);

        loop {
            match deployment.state() {
                DeploymentState::QUEUED => {
                    dbg!("job '{}' is queued", id);
                    deployment.update_state(DeploymentState::BUILDING);
                    let config = deployment.project_config();
                    match build_system.build(&crate_file, &config).await {
                        Ok(build) => {
                            deployment.attach_build_artifact(build);
                            deployment.update_state(DeploymentState::LOADING)
                        }
                        Err(_) => deployment.update_state(DeploymentState::ERROR)
                    }
                }
                DeploymentState::BUILDING => continue,
                DeploymentState::LOADING => {
                    dbg!("job '{}' is loading", id);
                    // todo load the dynamic library
                }
                DeploymentState::DEPLOYING => {
                    dbg!("job '{}' is deploying", id);
                    // todo update routing table
                }
                DeploymentState::READY => {
                    dbg!("job '{}' is ready", id);
                    break;
                }
                DeploymentState::CANCELLED => {
                    dbg!("job '{}' is cancelled", id);
                    break;
                }
                DeploymentState::ERROR => {
                    dbg!("job '{}' is errored", id);
                    break;
                }
            }
        }

        // load so file
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

