mod info;
mod queue;
mod run;
mod states;

pub use info::DeploymentInfo;
pub use states::DeploymentState;

pub use queue::Queued;
pub use run::Built;

use crate::persistence::Persistence;

use tokio::sync::{broadcast, mpsc};

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
    pub fn new(persistence: Persistence) -> Self {
        let (kill_send, _) = broadcast::channel(KILL_BUFFER_SIZE);

        DeploymentManager {
            pipeline: Pipeline::new(kill_send.clone(), persistence),
            kill_send,
        }
    }

    pub async fn queue_push(&self, queued: Queued) {
        self.pipeline.queue_send.send(queued).await.unwrap();
    }

    pub async fn run_push(&self, built: Built) {
        self.pipeline.run_send.send(built).await.unwrap();
    }

    pub async fn kill(&self, name: String) {
        if self.kill_send.receiver_count() > 0 {
            self.kill_send.send(name).unwrap();
        }
    }
}

/// ```
/// queue channel   all deployments here are DeploymentState::Queued
///       |
///       v
///  queue task     when taken from the channel by this task, deployments
///                 enter the DeploymentState::Building state and upon being
///       |         built transition to the DeploymentState::Built state
///       v
///  run channel    all deployments here are DeploymentState::Built
///       |
///       v
///    run task     tasks enter the DeploymentState::Running state and begin
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
    fn new(kill_send: KillSender, persistence: Persistence) -> Pipeline {
        let (queue_send, queue_recv) = mpsc::channel(QUEUE_BUFFER_SIZE);
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);

        let run_send_clone = run_send.clone();
        let persistence_clone = persistence.clone();

        tokio::spawn(async move { queue::task(queue_recv, run_send_clone, persistence).await });
        tokio::spawn(async move { run::task(run_recv, kill_send, persistence_clone).await });

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

type KillSender = broadcast::Sender<String>;
type KillReceiver = broadcast::Receiver<String>;
