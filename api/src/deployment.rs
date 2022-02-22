use std::collections::HashMap;
// use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
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
pub enum DeploymentStateLabel {
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
    state: DeploymentStateLabel,
    url: String,
    build_logs: Option<String>,
    runtime_logs: Option<String>,
}

pub(crate) struct DeploymentInner {
    info: DeploymentInfo,
    state: DeploymentState
}

pub(crate) struct Deployment {
    inner: RwLock<DeploymentInner>,
}

impl Deployment {
    pub(crate) async fn info(&self) -> DeploymentInfo {
        self.inner().await.info.clone()
    }

    pub(crate) async fn is_terminated(&self) -> bool {
        match self.inner().await.state {
            DeploymentState::QUEUED(_) | DeploymentState::BUILT(_) | DeploymentState::LOADED(_) => false,
            DeploymentState::DEPLOYED(_) | DeploymentState::ERROR => true,
        }
    }

    pub(crate) async fn advance(&self, build_system: Arc<Box<dyn BuildSystem>>) {
        let mut inner = self.inner_mut().await;
        match &inner.state {
            DeploymentState::QUEUED(queued) => {
                match build_system.build(&queued.crate_bytes, &self.info().await.config).await {
                    Ok(build) => inner.state = DeploymentState::built(build),
                    Err(_) => inner.state = DeploymentState::ERROR
                }
            },
            DeploymentState::BUILT(built) => {
                // let path = built.build.shared_object;
                // let (so, service) = service::load_service_from_so_file(path);
                inner.state = DeploymentState::loaded((), Box::new(()))
            },
            DeploymentState::LOADED(loaded) => {
                inner.state = DeploymentState::deployed((), Box::new(()), 0)
            }
            DeploymentState::DEPLOYED(_) => { /* nothing to do here */ }
            DeploymentState::ERROR => { /* nothing to do here */ }
        }
    }

    pub(crate) async fn update_state(&self, state: DeploymentStateLabel) {
        let mut inner = self.inner_mut().await;
        inner.info.state = state;
    }

    pub(crate) async fn project_config(&self) -> ProjectConfig {
        self.inner().await.info.config.clone()
    }

    // pub(crate) fn attach_build_artifact(&self, build: Build) {
    //     let mut inner = self.inner_mut();
    //     inner.build = Some(build)
    // }

    async fn inner_mut(&self) -> RwLockWriteGuard<'_, DeploymentInner> {
        self.inner.write().await
    }

    async fn inner(&self) -> RwLockReadGuard<'_, DeploymentInner> {
        self.inner.read().await
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

    // /// Get's the deployment information back to the user
    // pub(crate) fn get_deployment(&self, id: &DeploymentId) -> Result<DeploymentInfo, DeploymentError> {
    //     self.deployments
    //         .read()
    //         .unwrap()
    //         .get(&id)
    //         .map(|deployment| deployment.info())
    //         .ok_or(DeploymentError::NotFound("could not find deployment".to_string()))
    // }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) async fn deploy(&self,
                               crate_file: Data<'_>,
                               project_config: &ProjectConfig) -> Result<DeploymentInfo, DeploymentError> {

        let crate_bytes = crate_file
            .open(ByteUnit::max_value()).into_bytes()
            .await
            .map_err(|_| DeploymentError::BadRequest("could not read crate file into bytes".to_string()))?
            .to_vec();

        let info = DeploymentInfo {
            id: Uuid::new_v4(),
            config: project_config.clone(),
            state: DeploymentStateLabel::QUEUED,
            url: Self::create_url(project_config),
            build_logs: None,
            runtime_logs: None,
        };

        let id = info.id.clone();
        let queued_state = QueuedState {
            crate_bytes
        };

        let deployment = Deployment {
            inner: RwLock::new(DeploymentInner {
                info: info.clone(),
                state: DeploymentState::QUEUED(queued_state)
            })
        };

        let deployment = Arc::new(deployment);

        self.deployments
            .write()
            .await
            .insert(id.clone(), deployment.clone());

        let build_system = self.build_system.clone();
        let deployments = self.deployments.clone();


        tokio::spawn(async move {
            Self::start_deployment_job(
                build_system,
                deployment
            )
        });

        Ok(info)
    }

    async fn start_deployment_job(
        build_system: Arc<Box<dyn BuildSystem>>,
        deployment: Arc<Deployment>) {
        let id = deployment.info().await.id;

        dbg!("started deployment job for id: {}", id);

        while !deployment.is_terminated().await {
            deployment.advance(build_system.clone()).await
        }
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

// ---------


enum DeploymentState {
    QUEUED(QueuedState),
    BUILT(BuiltState),
    LOADED(LoadedState),
    DEPLOYED(DeployedState),
    ERROR
}

impl DeploymentState {
    fn queued(crate_bytes: Vec<u8>) -> Self {
        Self::QUEUED(
            QueuedState {
                crate_bytes
            }
        )
    }

    fn built(build: Build) -> Self {
        Self::BUILT(
            BuiltState {
                build
            }
        )
    }

    fn loaded(so: Library, service: Box<dyn Service>) -> Self {
        Self::LOADED(
            LoadedState {
                service,
                so
            }
        )
    }

    fn deployed(so: Library, service: Box<dyn Service>, port: u16) -> Self {
        Self::DEPLOYED(
            DeployedState {
                service,
                so,
                port
            }
        )
    }
}

struct QueuedState {
    crate_bytes: Vec<u8>
}

struct BuiltState {
    build: Build
}

struct LoadedState {
    service: Box<dyn Service>,
    so: Library
}

struct DeployedState {
    service: Box<dyn Service>,
    so: Library,
    port: u16
}
