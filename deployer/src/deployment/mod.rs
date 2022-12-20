pub mod deploy_layer;
pub mod gateway_client;
pub mod provisioner_factory;
mod queue;
mod run;
pub mod runtime_logger;
mod storage_manager;

use std::path::PathBuf;

pub use queue::Queued;
pub use run::{ActiveDeploymentsGetter, Built};
use tracing::{instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::persistence::{SecretRecorder, State};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use self::{
    deploy_layer::LogRecorder, gateway_client::BuildQueueClient, storage_manager::StorageManager,
};

const QUEUE_BUFFER_SIZE: usize = 100;
const RUN_BUFFER_SIZE: usize = 100;
const KILL_BUFFER_SIZE: usize = 10;

pub struct DeploymentManagerBuilder<AF, RLF, LR, SR, ADG, QC> {
    abstract_factory: Option<AF>,
    runtime_logger_factory: Option<RLF>,
    build_log_recorder: Option<LR>,
    secret_recorder: Option<SR>,
    active_deployment_getter: Option<ADG>,
    artifacts_path: Option<PathBuf>,
    queue_client: Option<QC>,
}

impl<AF, RLF, LR, SR, ADG, QC> DeploymentManagerBuilder<AF, RLF, LR, SR, ADG, QC>
where
    AF: provisioner_factory::AbstractFactory,
    RLF: runtime_logger::Factory,
    LR: LogRecorder,
    SR: SecretRecorder,
    ADG: ActiveDeploymentsGetter,
    QC: BuildQueueClient,
{
    pub fn abstract_factory(mut self, abstract_factory: AF) -> Self {
        self.abstract_factory = Some(abstract_factory);

        self
    }

    pub fn runtime_logger_factory(mut self, runtime_logger_factory: RLF) -> Self {
        self.runtime_logger_factory = Some(runtime_logger_factory);

        self
    }

    pub fn build_log_recorder(mut self, build_log_recorder: LR) -> Self {
        self.build_log_recorder = Some(build_log_recorder);

        self
    }

    pub fn secret_recorder(mut self, secret_recorder: SR) -> Self {
        self.secret_recorder = Some(secret_recorder);

        self
    }

    pub fn active_deployment_getter(mut self, active_deployment_getter: ADG) -> Self {
        self.active_deployment_getter = Some(active_deployment_getter);

        self
    }

    pub fn artifacts_path(mut self, artifacts_path: PathBuf) -> Self {
        self.artifacts_path = Some(artifacts_path);

        self
    }

    pub fn queue_client(mut self, queue_client: QC) -> Self {
        self.queue_client = Some(queue_client);

        self
    }

    /// Creates two Tokio tasks, one for building queued services, the other for
    /// executing/deploying built services. Two multi-producer, single consumer
    /// channels are also created which are for moving on-going service
    /// deployments between the aforementioned tasks.
    pub fn build(self) -> DeploymentManager {
        let abstract_factory = self
            .abstract_factory
            .expect("an abstract factory to be set");
        let runtime_logger_factory = self
            .runtime_logger_factory
            .expect("a runtime logger factory to be set");
        let build_log_recorder = self
            .build_log_recorder
            .expect("a build log recorder to be set");
        let secret_recorder = self.secret_recorder.expect("a secret recorder to be set");
        let active_deployment_getter = self
            .active_deployment_getter
            .expect("an active deployment getter to be set");
        let artifacts_path = self.artifacts_path.expect("artifacts path to be set");
        let queue_client = self.queue_client.expect("a queue client to be set");

        let (queue_send, queue_recv) = mpsc::channel(QUEUE_BUFFER_SIZE);
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);
        let (kill_send, _) = broadcast::channel(KILL_BUFFER_SIZE);
        let storage_manager = StorageManager::new(artifacts_path);

        let run_send_clone = run_send.clone();

        tokio::spawn(queue::task(
            queue_recv,
            run_send_clone,
            build_log_recorder,
            secret_recorder,
            storage_manager.clone(),
            queue_client,
        ));
        tokio::spawn(run::task(
            run_recv,
            kill_send.clone(),
            abstract_factory,
            runtime_logger_factory,
            active_deployment_getter,
            storage_manager.clone(),
        ));

        DeploymentManager {
            queue_send,
            run_send,
            kill_send,
            storage_manager,
        }
    }
}

#[derive(Clone)]
pub struct DeploymentManager {
    queue_send: QueueSender,
    run_send: RunSender,
    kill_send: KillSender,
    storage_manager: StorageManager,
}

/// ```no-test
/// queue channel   all deployments here are State::Queued until the get a slot from gateway
///       |
///       v
///  queue task     when taken from the channel by this task, deployments
///                 enter the State::Building state and upon being
///       |         built transition to the State::Built state
///       v
///  run channel    all deployments here are State::Built
///       |
///       v
///    run task     tasks enter the State::Running state and begin
///                 executing
/// ```
impl DeploymentManager {
    /// Create a new deployment manager. Manages one or more 'pipelines' for
    /// processing service building, loading, and deployment.
    pub fn builder<AF, RLF, LR, SR, ADG, QC>() -> DeploymentManagerBuilder<AF, RLF, LR, SR, ADG, QC>
    {
        DeploymentManagerBuilder {
            abstract_factory: None,
            runtime_logger_factory: None,
            build_log_recorder: None,
            secret_recorder: None,
            active_deployment_getter: None,
            artifacts_path: None,
            queue_client: None,
        }
    }

    pub async fn queue_push(&self, mut queued: Queued) {
        let cx = Span::current().context();

        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut queued.tracing_context);
        });

        self.queue_send.send(queued).await.unwrap();
    }

    #[instrument(skip(self), fields(id = %built.id, state = %State::Built))]
    pub async fn run_push(&self, built: Built) {
        self.run_send.send(built).await.unwrap();
    }

    pub async fn kill(&self, id: Uuid) {
        if self.kill_send.receiver_count() > 0 {
            self.kill_send.send(id).unwrap();
        }
    }

    pub fn storage_manager(&self) -> StorageManager {
        self.storage_manager.clone()
    }
}

type QueueSender = mpsc::Sender<queue::Queued>;
type QueueReceiver = mpsc::Receiver<queue::Queued>;

type RunSender = mpsc::Sender<run::Built>;
type RunReceiver = mpsc::Receiver<run::Built>;

type KillSender = broadcast::Sender<Uuid>;
type KillReceiver = broadcast::Receiver<Uuid>;
