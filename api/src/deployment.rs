use core::default::Default;
use libloading::Library;
use rocket::data::ByteUnit;
use rocket::tokio;
use rocket::Data;
use std::collections::HashMap;
use std::error::Error;
use std::fs::DirEntry;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::{anyhow, Context as AnyhowContext};
use tokio::task::JoinHandle;
use tokio::sync::RwLock;
use crate::build::Build;
use crate::{BuildSystem, UnveilFactory};
use lib::{DeploymentApiError, DeploymentId, DeploymentMeta, DeploymentStateMeta, Host, Port};
use lib::project::ProjectConfig;

use crate::database;
use crate::router::Router;
use unveil_service::Service;

/// Inner struct of a deployment which holds the deployment itself
/// and the some metadata
pub(crate) struct Deployment {
    meta: Arc<RwLock<DeploymentMeta>>,
    state: RwLock<DeploymentState>,
}

impl Deployment {
    fn new(meta: DeploymentMeta, state: DeploymentState) -> Self {
        Self {
            meta: Arc::new(RwLock::new(meta)),
            state: RwLock::new(state)
        }
    }

    fn from_bytes(config: &ProjectConfig, crate_bytes: Vec<u8>) -> Self {
        Self {
            meta: Arc::new(RwLock::new(DeploymentMeta::queued(&config))),
            state: RwLock::new(DeploymentState::queued(crate_bytes)),
        }
    }

    /// Initialise a deployment from a directory
    fn from_directory(dir: DirEntry) -> Result<Self, anyhow::Error> {
        let project_path = dir.path();
        let project_name = dir.file_name()
            .into_string()
            .map_err(|os_str| anyhow!("could not parse project name `{:?}` to string", os_str))?;
        // find marker which points to so file
        let marker_path = project_path.join(".unveil_marker");
        let so_path_str = std::fs::read(&marker_path)
            .context(anyhow!("could not find so marker file at {:?}", marker_path))?;
        let so_path: PathBuf = String::from_utf8_lossy(&so_path_str)
            .parse()
            .context("could not parse contents of marker file to a valid path")?;

        let meta = DeploymentMeta::built(&ProjectConfig::new(project_name)?);
        let state = DeploymentState::built(Build { so_path });
        Ok(Self::new(meta, state))
    }

    /// Gets a `clone`ed copy of the metadata.
    pub(crate) async fn meta(&self) -> DeploymentMeta {
        log::debug!("trying to get meta");
        self.meta.read().await.clone()
    }

    /// Evaluates if the deployment can be advanced. If the deployment
    /// has reached a state where it can no longer `advance`, returns `false`.
    pub(crate) async fn deployment_finished(&self) -> bool {
        match *self.state.read().await {
            DeploymentState::Queued(_) | DeploymentState::Built(_) | DeploymentState::Loaded(_) => {
                false
            }
            DeploymentState::Deployed(_) | DeploymentState::Error(_) | DeploymentState::Deleted => true,
        }
    }

    /// Tries to advance the deployment one stage. Does nothing if the deployment
    /// is in a terminal state.
    pub(crate) async fn advance(&self, context: &Context, db_context: &database::Context) {
        {
            log::debug!("waiting to get write on the state");
            let meta = self.meta().await;
            let mut state = self.state.write().await;

            *state = match state.take() {
                DeploymentState::Queued(queued) => {
                    log::debug!("deployment '{}' build starting...", &meta.id);

                    let console_writer = BuildOutputWriter::new(self.meta.clone());
                    match context
                        .build_system
                        .build(&queued.crate_bytes, &meta.config, Box::new(console_writer))
                        .await
                    {
                        Ok(build) => DeploymentState::built(build),
                        Err(e) => {
                            dbg!("failed to build with error: {}", &e);
                            DeploymentState::Error(e)
                        }
                    }
                }
                DeploymentState::Built(built) => {
                    log::debug!(
                        "deployment '{}' loading shared object and service...",
                        &meta.id
                    );

                    match load_service_from_so_file(&built.build.so_path) {
                        Ok((svc, so)) => DeploymentState::loaded(so, svc),
                        Err(e) => {
                            log::debug!("failed to load with error: {}", &e);
                            DeploymentState::Error(e)
                        }
                    }
                }
                DeploymentState::Loaded(mut loaded) => {
                    let port = identify_free_port();

                    log::debug!(
                        "deployment '{}' getting deployed on port {}...",
                        meta.id,
                        port
                    );

                    let mut db_state = database::State::default();

                    let factory =
                        UnveilFactory::new(&mut db_state, meta.config.clone(), db_context.clone());

                    match loaded.service.build(&factory).await {
                        Err(e) => DeploymentState::Error(e.into()),
                        Ok(_) => {
                            let serve_task = loaded
                                .service
                                .bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port));

                            // TODO: upon resolving this future, change the status of the deployment
                            let handle = tokio::spawn(serve_task);

                            // Remove stale active deployments
                            if let Some(stale_id) = context.router.promote(meta.host, meta.id).await {
                                log::debug!("removing stale deployment `{}`", &stale_id);
                                context.deployments.write().await.remove(&stale_id);
                            }

                            DeploymentState::deployed(loaded.so, port, handle, db_state)

                        }
                    }
                }
                deployed_or_error => deployed_or_error, /* nothing to do here */
            };
        }

        // ensures that the metadata state is inline with the actual state. This
        // can go when we have an API layer.
        self.update_meta_state().await;
    }

    async fn update_meta_state(&self) {
        self.meta.write().await.state = self.state.read().await.meta()
    }

    async fn port(&self) -> Option<Port> {
        match &*self.state.read().await {
            DeploymentState::Deployed(deployed) => Some(deployed.port),
            _ => None,
        }
    }
}

/// Provides a `Write` wrapper around the build logs - i.e., the build output
/// is written into our build logs using this wrapper.
struct BuildOutputWriter {
    meta: Arc<RwLock<DeploymentMeta>>,
    buf: String,
}

impl BuildOutputWriter {
    pub fn new(meta: Arc<RwLock<DeploymentMeta>>) -> Self {
        Self {
            meta,
            buf: String::new(),
        }
    }
}

impl Write for BuildOutputWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let write_len = buf.len();
        let is_new_line = buf[write_len - 1] == b'\n';
        if let Ok(buf) = std::str::from_utf8(buf) {
            self.buf.push_str(buf);

            // The flush step introduces async code which can potentially execute out of order.
            // For example, if the write is called with inputs a, b, and c then a new threads will be
            // spawned to add a, b, and c to meta. However, the threads might execute in others b, a,
            // and c if they are too close to one another. This `write` method seems to be called for
            // every character which causes many threads with overlapping lifetimes and therefore
            // many out of order executions which just mess up the log output.
            // Since line orders rarely matter and only spawning a thread for each output line also
            // reduces the likelihood of threads with overlapping lifetimes, the guard exists.
            if is_new_line {
                // Safe to unwrap since `flush` has no errors internally
                self.flush().unwrap();
            }

            return Ok(write_len);
        }

        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let handle = tokio::runtime::Handle::current();
        let meta = self.meta.clone();
        let buf = self.buf.clone();
        self.buf = String::new();

        // Can't call `block_on` on a thread that already has a tokio executor, so spawn a new one
        std::thread::spawn(move || {
            handle.block_on(async {
                meta.write()
                    .await
                    .build_logs
                    .get_or_insert("".to_string())
                    .push_str(&buf)
            });
        });

        Ok(())
    }
}

// Make sure to clean the buffer one last time
impl Drop for BuildOutputWriter {
    fn drop(&mut self) {
        self.flush().unwrap();
    }
}

type Deployments = HashMap<DeploymentId, Arc<Deployment>>;

/// The top-level manager for deployments. Is responsible for their creation
/// and lifecycle.
pub(crate) struct DeploymentSystem {
    deployments: Arc<RwLock<Deployments>>,
    job_queue: Arc<JobQueue>,
    router: Arc<Router>,
}

#[derive(Default)]
struct JobQueue {
    queue: Arc<Mutex<Vec<Arc<Deployment>>>>,
}

impl JobQueue {
    fn push(&self, deployment: Arc<Deployment>) {
        self.queue.lock().unwrap().push(deployment)
    }

    fn pop(&self) -> Option<Arc<Deployment>> {
        self.queue.lock().unwrap().pop()
    }

    /// Returns a JobQueue with the job processor already running
    async fn initialise(context: Context, db_context: database::Context) -> Arc<Self> {
        let job_queue = Arc::new(JobQueue::default());

        let queue_ref = job_queue.clone();

        tokio::spawn(
            async move { Self::start_job_processor(context, db_context, queue_ref).await },
        );

        job_queue
    }

    /// Starts the job processor. Before it begins, it will add all projects
    /// from the deployment service into the queue.
    async fn start_job_processor(
        context: Context,
        db_context: database::Context,
        queue: Arc<JobQueue>,
    ) {
        log::debug!("loading deployments into job processor");
        for deployment in context.deployments.read().await.values() {
            queue.push(deployment.clone());
            log::debug!("loading deployment: {:?}", deployment.meta);
        }
        log::debug!("starting job processor loop");
        loop {
            if let Some(deployment) = queue.pop() {
                let id = deployment.meta().await.id;

                log::debug!("started deployment job for id: '{}'", id);

                while !deployment.deployment_finished().await {
                    deployment.advance(&context, &db_context).await;
                }

                log::debug!("ended deployment job for id: '{}'", id);
            } else {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
        }
    }
}

/// Convenience struct used to store a bunch of stuff needed for the job
/// processor.
pub(crate) struct Context {
    router: Arc<Router>,
    build_system: Box<dyn BuildSystem>,
    deployments: Arc<RwLock<Deployments>>,
}

impl DeploymentSystem {
    pub(crate) async fn new(build_system: Box<dyn BuildSystem>) -> Self {
        let router: Arc<Router> = Default::default();

        let deployments = Arc::new(
            RwLock::new(
                Self::initialise_from_fs(&build_system.fs_root()).await
            )
        );

        let context = Context {
            router: router.clone(),
            build_system,
            deployments: deployments.clone(),
        };
        let db_context = database::Context::new()
            .await
            .expect("failed to create lazy connection to database");

        Self {
            deployments,
            job_queue: JobQueue::initialise(context, db_context).await,
            router,
        }
    }

    /// Traverse the build directory re-create deployments.
    /// If a project could not be re-created, this will get logged and skipped.
    async fn initialise_from_fs(fs_root: &Path) -> Deployments {
        let mut deployments = HashMap::default();
        for project_dir in std::fs::read_dir(fs_root)
            .unwrap() // safety: api can read the fs root dir
            .into_iter() {
            let project_dir = match project_dir {
                Ok(project_dir) => project_dir,
                Err(e) => {
                    log::warn!("failed to read directory for project with error `{:?}`", e);
                    log::warn!("skipping...");
                    continue;
                }
            };
            let project_name = project_dir.file_name();
            match Deployment::from_directory(project_dir) {
                Err(e) => {
                    log::warn!(
                        "failed to re-create deployment for project `{:?}` with error: {:?}",
                        project_name,
                        e
                    );
                }
                Ok(deployment) => {
                    let deployment = Arc::new(deployment);
                    let info = deployment.meta().await;
                    deployments.insert(info.id.clone(), deployment.clone());
                }
            }
        }
        deployments
    }


    /// Returns the port for a given host. If the host does not exist, returns
    /// `None`.
    pub(crate) async fn port_for_host(&self, host: &Host) -> Option<Port> {
        let id_for_host = self.router.id_for_host(host).await?;
        self.deployments
            .read()
            .await
            .get(&id_for_host)?
            .port()
            .await
    }

    /// Retrieves a clone of the deployment information
    pub(crate) async fn get_deployment(
        &self,
        id: &DeploymentId,
    ) -> Result<DeploymentMeta, DeploymentApiError> {
        match self.deployments.read().await.get(id) {
            Some(deployment) => Ok(deployment.meta().await),
            None => Err(DeploymentApiError::NotFound(format!(
                "could not find deployment for id '{}'",
                &id
            ))),
        }
    }

    /// Retrieves a clone of the deployment information
    /// for a given project. If there are multiple deployments
    /// for a given project, will return the latest.
    pub(crate) async fn get_deployment_for_project(
        &self,
        project_name: &str,
    ) -> Result<DeploymentMeta, DeploymentApiError> {
        let mut candidates = Vec::new();

        for deployment in self.deployments.read().await.values() {
            if deployment.meta.read().await.config.name() == project_name {
                candidates.push(deployment.meta().await);
            }
        }

        let latest = candidates
            .into_iter()
            .max_by(|d1, d2| d1.created_at.cmp(&d2.created_at));

        match latest {
            Some(latest) => Ok(latest),
            None => Err(DeploymentApiError::NotFound(format!(
                "could not find deployment for project '{}'",
                &project_name
            ))),
        }
    }

    pub(crate) async fn kill_deployment_for_project(
        &self,
        project_name: &str,
    ) -> Result<DeploymentMeta, DeploymentApiError> {
        let id = self.get_deployment_for_project(project_name).await?.id;
        self.kill_deployment(&id).await
    }

    /// Remove a deployment from the deployments hash map and, if it has
    /// already been deployed, kill the Tokio task in which it is running
    /// and deallocate the linked library.
    pub(crate) async fn kill_deployment(
        &self,
        id: &DeploymentId,
    ) -> Result<DeploymentMeta, DeploymentApiError> {
        match self.deployments.write().await.remove(id) {
            Some(deployment) => {
                let mut meta = deployment.meta().await;

                // If the deployment is in the 'deployed' state, kill the Tokio
                // task in which it is deployed and deallocate the linked
                // library when the runtime gets around to it.

                let mut lock = deployment.state.write().await;
                if let DeploymentState::Deployed(DeployedState { so, handle, .. }) = lock.take() {
                    handle.abort();
                    tokio::spawn(async move {
                        so.close().unwrap();
                    });
                }

                let _ = self.router.remove(&meta.host);

                meta.state = DeploymentStateMeta::Deleted;

                Ok(meta)
            }
            None => Err(DeploymentApiError::NotFound(String::new())),
        }
    }

    /// Main way to interface with the deployment manager.
    /// Will take a crate through the whole lifecycle.
    pub(crate) async fn deploy(
        &self,
        crate_file: Data<'_>,
        project_config: &ProjectConfig,
    ) -> Result<DeploymentMeta, DeploymentApiError> {
        let crate_bytes = crate_file
            .open(ByteUnit::max_value())
            .into_bytes()
            .await
            .map_err(|_| {
                DeploymentApiError::BadRequest("could not read crate file into bytes".to_string())
            })?
            .to_vec();

        let deployment = Arc::new(Deployment::from_bytes(&project_config, crate_bytes));
      
        let info = deployment.meta().await;

        self.deployments
            .write()
            .await
            .insert(info.id, deployment.clone());

        self.add_to_job_queue(deployment);

        Ok(info)
    }

    fn add_to_job_queue(&self, deployment: Arc<Deployment>) {
        self.job_queue.push(deployment)
    }
}

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type ServeHandle = JoinHandle<Result<(), unveil_service::Error>>;

type CreateService = unsafe extern "C" fn() -> *mut dyn Service;

/// Dynamically load from a `.so` file a value of a type implementing the
/// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
/// function called [`ENTRYPOINT_SYMBOL_NAME`], likely automatically generated
/// using the [`unveil_service::declare_service`] macro.
#[allow(clippy::type_complexity)]
fn load_service_from_so_file(so_path: &Path) -> anyhow::Result<(Box<dyn Service>, Library)> {
    unsafe {
        let lib = libloading::Library::new(so_path)?;

        let entrypoint: libloading::Symbol<CreateService> = lib.get(ENTRYPOINT_SYMBOL_NAME)?;
        let raw = entrypoint();

        Ok((Box::from_raw(raw), lib))
    }
}

/// Call on the operating system to identify an available port on which a
/// deployment may then be hosted.
fn identify_free_port() -> u16 {
    let ip = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    TcpListener::bind(ip).unwrap().local_addr().unwrap().port()
}

/// Finite-state machine representing the various stages of the build
/// process.
enum DeploymentState {
    /// Deployment waiting to be built.
    Queued(QueuedState),
    /// Built deployment that is ready and waiting to be loaded.
    Built(BuiltState),
    /// Deployment is loaded into the server application as a
    /// dynamically-linked library (`.so` file). The [`libloading`] crate has
    /// been used to achieve this and to obtain this particular deployment's
    /// implementation of the [`unveil_service::Service`] trait.
    Loaded(LoadedState),
    /// Deployment that is actively running inside a Tokio task and listening
    /// for connections on some port indicated in [`DeployedState`].
    Deployed(DeployedState),
    /// A state entered when something unexpected occurs during the deployment
    /// process.
    Error(anyhow::Error),
    /// A state indicating that the user has intentionally terminated this
    /// deployment
    #[allow(dead_code)]
    Deleted
}

impl DeploymentState {
    fn take(&mut self) -> Self {
        std::mem::replace(self, DeploymentState::Deleted)
    }

    fn queued(crate_bytes: Vec<u8>) -> Self {
        Self::Queued(QueuedState { crate_bytes })
    }

    fn built(build: Build) -> Self {
        Self::Built(BuiltState { build })
    }

    fn loaded(so: Library, service: Box<dyn Service>) -> Self {
        Self::Loaded(LoadedState { service, so })
    }

    fn deployed(
        so: Library,
        port: Port,
        handle: ServeHandle,
        db_state: database::State,
    ) -> Self {
        Self::Deployed(DeployedState {
            so,
            port,
            handle,
            db_state,
        })
    }

    fn meta(&self) -> DeploymentStateMeta {
        match self {
            DeploymentState::Queued(_) => DeploymentStateMeta::Queued,
            DeploymentState::Built(_) => DeploymentStateMeta::Built,
            DeploymentState::Loaded(_) => DeploymentStateMeta::Loaded,
            DeploymentState::Deployed(_) => DeploymentStateMeta::Deployed,
            DeploymentState::Error(e) => DeploymentStateMeta::Error(format!("{:#?}", e)),
            DeploymentState::Deleted => DeploymentStateMeta::Deleted
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

#[allow(dead_code)]
struct DeployedState {
    so: Library,
    port: Port,
    handle: ServeHandle,
    db_state: database::State,
}
