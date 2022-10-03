pub mod deploy_layer;
pub mod provisioner_factory;
mod queue;
mod run;
pub mod runtime_logger;

pub use queue::Queued;
pub use run::{ActiveDeploymentsGetter, Built};
use tracing::instrument;

use crate::persistence::{SecretRecorder, State};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use self::deploy_layer::LogRecorder;

const QUEUE_BUFFER_SIZE: usize = 100;
const RUN_BUFFER_SIZE: usize = 100;
const KILL_BUFFER_SIZE: usize = 10;

#[derive(Clone)]
pub struct DeploymentManager {
    pipeline: Pipeline,
    kill_send: KillSender,
}

impl DeploymentManager {
    /// Create a new deployment manager. Manages one or more 'pipelines' for
    /// processing service building, loading, and deployment.
    pub fn new(
        abstract_factory: impl provisioner_factory::AbstractFactory,
        runtime_logger_factory: impl runtime_logger::Factory,
        build_log_recorder: impl LogRecorder,
        secret_recorder: impl SecretRecorder,
        active_deployment_getter: impl ActiveDeploymentsGetter,
    ) -> Self {
        let (kill_send, _) = broadcast::channel(KILL_BUFFER_SIZE);

        DeploymentManager {
            pipeline: Pipeline::new(
                kill_send.clone(),
                abstract_factory,
                runtime_logger_factory,
                build_log_recorder,
                secret_recorder,
                active_deployment_getter,
            ),
            kill_send,
        }
    }

    #[instrument(skip(self), fields(id = %queued.id, state = %State::Queued))]
    pub async fn queue_push(&self, queued: Queued) {
        self.pipeline.queue_send.send(queued).await.unwrap();
    }

    #[instrument(skip(self), fields(id = %built.id, state = %State::Built))]
    pub async fn run_push(&self, built: Built) {
        self.pipeline.run_send.send(built).await.unwrap();
    }

    pub async fn kill(&self, id: Uuid) {
        if self.kill_send.receiver_count() > 0 {
            self.kill_send.send(id).unwrap();
        }
    }
}

/// ```no-test
/// queue channel   all deployments here are State::Queued
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
#[derive(Clone)]
struct Pipeline {
    queue_send: QueueSender,
    run_send: RunSender,
}

impl Pipeline {
    /// Creates two Tokio tasks, one for building queued services, the other for
    /// executing/deploying built services. Two multi-producer, single consumer
    /// channels are also created which are for moving on-going service
    /// deployments between the aforementioned tasks.
    fn new(
        kill_send: KillSender,
        abstract_factory: impl provisioner_factory::AbstractFactory,
        runtime_logger_factory: impl runtime_logger::Factory,
        build_log_recorder: impl LogRecorder,
        secret_recorder: impl SecretRecorder,
        active_deployment_getter: impl ActiveDeploymentsGetter,
    ) -> Pipeline {
        let (queue_send, queue_recv) = mpsc::channel(QUEUE_BUFFER_SIZE);
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);

        let run_send_clone = run_send.clone();

        tokio::spawn(queue::task(
            queue_recv,
            run_send_clone,
            build_log_recorder,
            secret_recorder,
        ));
        tokio::spawn(run::task(
            run_recv,
            kill_send,
            abstract_factory,
            runtime_logger_factory,
            active_deployment_getter,
        ));

        Pipeline {
            queue_send,
            run_send,
        }
    }
}

type QueueSender = mpsc::Sender<queue::Queued>;
type QueueReceiver = mpsc::Receiver<queue::Queued>;

type RunSender = mpsc::Sender<run::Built>;
type RunReceiver = mpsc::Receiver<run::Built>;

type KillSender = broadcast::Sender<Uuid>;
type KillReceiver = broadcast::Receiver<Uuid>;
