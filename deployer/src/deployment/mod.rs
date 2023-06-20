pub mod driver;
pub mod error;

use shuttle_common::{claims::Claim, storage_manager::ArtifactsStorageManager};
use std::path::PathBuf;
use tracing::instrument;
use ulid::Ulid;

use crate::dal::Dal;
use crate::runtime_manager::RuntimeManager;
use driver::RunnableDeployment;
use tokio::sync::mpsc;

const RUN_BUFFER_SIZE: usize = 100;

pub struct DeploymentManagerBuilder<D: Dal + Sync + 'static> {
    artifacts_path: Option<PathBuf>,
    runtime_manager: Option<RuntimeManager>,
    dal: Option<D>,
    claim: Option<Claim>,
}

impl<D: Dal + Send + Sync + 'static> DeploymentManagerBuilder<D> {
    pub fn dal(mut self, dal: D) -> Self {
        self.dal = Some(dal);

        self
    }

    pub fn artifacts_path(mut self, artifacts_path: PathBuf) -> Self {
        self.artifacts_path = Some(artifacts_path);

        self
    }

    pub fn claim(mut self, claim: Claim) -> Self {
        self.claim = Some(claim);
        self
    }

    pub fn runtime(mut self, runtime_manager: RuntimeManager) -> Self {
        self.runtime_manager = Some(runtime_manager);

        self
    }

    pub fn build(self) -> DeploymentManager<D> {
        let artifacts_path = self.artifacts_path.expect("artifacts path to be set");
        let runtime_manager = self.runtime_manager.expect("a runtime manager to be set");
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);
        let storage_manager = ArtifactsStorageManager::new(artifacts_path);
        let dal = self.dal.expect("a DAL is required");

        tokio::spawn(driver::task(
            dal.clone(),
            run_recv,
            runtime_manager.clone(),
            storage_manager.clone(),
            self.claim,
        ));

        DeploymentManager {
            run_send,
            runtime_manager,
            storage_manager,
            dal,
        }
    }
}

#[derive(Clone)]
pub struct DeploymentManager<D: Dal + Sync + 'static> {
    run_send: RunSender,
    runtime_manager: RuntimeManager,
    storage_manager: ArtifactsStorageManager,
    dal: D,
}

/// ```no-test
///  run channel    all deployments here have a manifest coming from the shuttle-builder
///       |
///       v
///    run task     tasks load and start the shuttle-runtimes that are started on a separate
///                 worker
/// ```
impl<D: Dal + Sync + 'static> DeploymentManager<D> {
    /// Create a new deployment manager. Manages one or more 'pipelines' for
    /// processing service loading and starting.
    pub fn builder() -> DeploymentManagerBuilder<D> {
        DeploymentManagerBuilder {
            artifacts_path: None,
            runtime_manager: None,
            dal: None,
            claim: None,
        }
    }

    async fn run_push(&self, run: RunnableDeployment) -> Result<(), error::Error> {
        self.run_send
            .send(run)
            .await
            .map_err(|err| error::Error::Send(err.to_string()))
    }

    #[instrument(skip(self), fields(service_id = %service_id))]
    pub async fn run_deployment(
        &self,
        service_id: Ulid,
        deployment_id: Ulid,
        network_name: &str,
        claim: Option<Claim>,
        is_next: bool,
    ) -> Result<(), error::Error> {
        // Refreshing the container should restart it and persist a new associated address to it.
        let service = self
            .dal
            .service(&service_id)
            .await
            .map_err(error::Error::Dal)?;

        let run = RunnableDeployment {
            deployment_id,
            service_name: service.name,
            service_id: service.id,
            tracing_context: Default::default(),
            claim,
            target_ip: service
                .state
                .target_ip(network_name)
                .map_err(|_| error::Error::MissingIpv4Address)?,
            is_next,
        };

        self.run_push(run).await
    }

    pub async fn kill(&mut self, id: Ulid) {
        self.runtime_manager.kill(&id).await;
    }

    pub fn storage_manager(&self) -> ArtifactsStorageManager {
        self.storage_manager.clone()
    }
}

type RunSender = mpsc::Sender<RunnableDeployment>;
