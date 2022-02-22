use std::collections::HashMap;
// use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::path::Path;
use libloading::Library;
use rocket::{Data};
use rocket::data::ByteUnit;
use rocket::response::Responder;
use uuid::Uuid;
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio;

use crate::{BuildSystem, ProjectConfig};
use crate::build::Build;

use service::Service;

pub type DeploymentId = Uuid;

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

/// Deployment metadata. This serves two purposes. Storing information
/// used for the deployment process and also providing the client with
/// information on the state of the deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMeta {
    id: DeploymentId,
    config: ProjectConfig,
    state: DeploymentStateMeta,
    url: String,
    build_logs: Option<String>,
    runtime_logs: Option<String>,
}

impl DeploymentMeta {
    fn new(config: &ProjectConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config: config.clone(),
            state: DeploymentStateMeta::QUEUED,
            url: Self::create_url(config),
            build_logs: None,
            runtime_logs: None,
        }
    }

    fn create_url(project_config: &ProjectConfig) -> String {
        format!("{}.unveil.sh", project_config.name)
    }
}

/// A wrapper struct for encapsulation and interior mutability
pub(crate) struct Deployment(RwLock<DeploymentInner>);

/// Inner struct of a deployment which holds the deployment itself
/// and the some metadata
pub(crate) struct DeploymentInner {
    info: DeploymentMeta,
    state: DeploymentState,
}

impl Deployment {
    fn new(config: &ProjectConfig, crate_bytes: Vec<u8>) -> Self {
        Self(
            RwLock::new(DeploymentInner {
                info: DeploymentMeta::new(&config),
                state: DeploymentState::queued(crate_bytes),
            })
        )
    }
}

impl Deployment {
    pub(crate) async fn info(&self) -> DeploymentMeta {
        dbg!("trying to get info");
        self.inner().await.info.clone()
    }

    /// Evaluates if the deployment can be advanced. If the deployment
    /// has reached a state where it can no longer advance, returns `false`.
    pub(crate) async fn deployment_finished(&self) -> bool {
        match self.inner().await.state {
            DeploymentState::QUEUED(_) | DeploymentState::BUILT(_) | DeploymentState::LOADED(_) => false,
            DeploymentState::DEPLOYED(_) | DeploymentState::ERROR => true,
        }
    }

    /// Tries to advance the deployment one stage. Does nothing if the deployment
    /// is in a terminal state.
    pub(crate) async fn advance(&self, build_system: Arc<Box<dyn BuildSystem>>) {
        /// We get the project first so that we don't deadlock
        let config = self.project_config().await;
        let mut inner = self.inner_mut().await;
        let id = inner.info.id.clone();
        match &inner.state {
            DeploymentState::QUEUED(queued) => {
                dbg!("deployment '{}' build starting...", &id);
                match build_system.build(&queued.crate_bytes, &config).await {
                    Ok(build) => inner.state = DeploymentState::built(build),
                    Err(_) => inner.state = DeploymentState::ERROR
                }
            }
            DeploymentState::BUILT(built) => {
                dbg!("deployment '{}' loading shared object and service...", &id);
                match load_service_from_so_file(&built.build.so_path) {
                    Ok((svc, so)) => inner.state = DeploymentState::loaded(so, svc),
                    Err(_) => inner.state = DeploymentState::ERROR
                }
            }
            DeploymentState::LOADED(_loaded) => {
                dbg!("deployment '{}' getting deployed...", &id);
                todo!("functionality to load service objects not ready")
            }
            DeploymentState::DEPLOYED(_) => { /* nothing to do here */ }
            DeploymentState::ERROR => { /* nothing to do here */ }
        }
        // ensures that the metadata state is inline with the actual
        // state. This can go when we have an API layer.
        self.update_meta_state().await
    }

    pub(crate) async fn project_config(&self) -> ProjectConfig {
        self.inner().await.info.config.clone()
    }

    async fn inner_mut(&self) -> RwLockWriteGuard<'_, DeploymentInner> {
        self.0.write().await
    }

    async fn inner(&self) -> RwLockReadGuard<'_, DeploymentInner> {
        self.0.read().await
    }

    async fn update_meta_state(&self) {
        let mut inner = self.inner_mut().await;
        inner.info.state = inner.state.meta()
    }
}

type Deployments = HashMap<DeploymentId, Arc<Deployment>>;

/// The top-level manager for deployments. Is responsible for their
/// creation and lifecycle.
pub(crate) struct DeploymentSystem {
    build_system: Arc<Box<dyn BuildSystem>>,
    deployments: RwLock<Deployments>,
}

impl DeploymentSystem {
    pub(crate) fn new(build_system: Box<dyn BuildSystem>) -> Self {
        Self {
            build_system: Arc::new(build_system),
            deployments: Default::default(),
        }
    }

    /// Retrieves a clone of the deployment information
    pub(crate) async fn get_deployment(&self, id: &DeploymentId) -> Result<DeploymentMeta, DeploymentError> {
        match self.deployments.read().await.get(&id) {
            Some(deployment) => Ok(deployment.info().await),
            None => Err(DeploymentError::NotFound(format!("could not find deployment for id '{}'", &id)))
        }
    }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) async fn deploy(&self,
                               crate_file: Data<'_>,
                               project_config: &ProjectConfig) -> Result<DeploymentMeta, DeploymentError> {
        let crate_bytes = crate_file
            .open(ByteUnit::max_value()).into_bytes()
            .await
            .map_err(|_| DeploymentError::BadRequest("could not read crate file into bytes".to_string()))?
            .to_vec();

        let deployment = Arc::new(Deployment::new(&project_config, crate_bytes));
        let info = deployment.info().await;
        let id = info.id.clone();

        self.deployments
            .write()
            .await
            .insert(info.id.clone(), deployment.clone());

        let build_system = self.build_system.clone();

        tokio::spawn(async move {
            Self::start_deployment_job(
                build_system,
                deployment,
            ).await
        });

        Ok(info)
    }

    async fn start_deployment_job(
        build_system: Arc<Box<dyn BuildSystem>>,
        deployment: Arc<Deployment>) {
        dbg!("function called");

        let id = deployment.info().await.id;

        dbg!("started deployment job for id: '{}'", id);

        while !deployment.deployment_finished().await {
            deployment.advance(build_system.clone()).await
        }

        dbg!("ended deployment job for id: '{}'", id);
    }
}

const ENTRYPOINT_SYMBOL_NAME: &'static [u8] = b"_create_service\0";

type CreateService = unsafe extern fn() -> *mut dyn Service;

/// Dynamically load from a `.so` file a value of a type implementing the
/// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
/// function called [`ENTRYPOINT_SYMBOL_NAME`], likely automatically generated
/// using the [`service::declare_service`] macro.
fn load_service_from_so_file(so_path: &Path) -> anyhow::Result<(Box<dyn Service>, Library)> {
    unsafe {
        let lib = libloading::Library::new(so_path)?;

        let entrypoint: libloading::Symbol<CreateService> = lib.get(ENTRYPOINT_SYMBOL_NAME)?;
        let raw = entrypoint();

        Ok((Box::from_raw(raw), lib))
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

/// Finite-state machine representing the various stages of the build
/// process.
enum DeploymentState {
    QUEUED(QueuedState),
    BUILT(BuiltState),
    LOADED(LoadedState),
    DEPLOYED(DeployedState),
    ERROR,
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
                so,
            }
        )
    }

    fn deployed(so: Library, service: Box<dyn Service>, port: u16) -> Self {
        Self::DEPLOYED(
            DeployedState {
                service,
                so,
                port,
            }
        )
    }

    fn meta(&self) -> DeploymentStateMeta {
        match self {
            DeploymentState::QUEUED(_) => DeploymentStateMeta::QUEUED,
            DeploymentState::BUILT(_) => DeploymentStateMeta::BUILT,
            DeploymentState::LOADED(_) => DeploymentStateMeta::LOADED,
            DeploymentState::DEPLOYED(_) => DeploymentStateMeta::DEPLOYED,
            DeploymentState::ERROR => DeploymentStateMeta::ERROR
        }
    }
}

struct QueuedState {
    crate_bytes: Vec<u8>,
}

struct BuiltState {
    build: Build,
}

struct LoadedState {
    service: Box<dyn Service>,
    so: Library,
}

struct DeployedState {
    service: Box<dyn Service>,
    so: Library,
    port: u16,
}
