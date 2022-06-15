mod info;
mod queue;
mod run;
mod states;

pub use info::DeploymentInfo;
pub use states::DeploymentState;

pub use queue::Queued;
pub use run::Built;

use std::iter;
use std::sync::Arc;

use crate::persistence::Persistence;

use tokio::{sync::mpsc, task::JoinHandle};

const QUEUE_BUFFER_SIZE: usize = 100;
const RUN_BUFFER_SIZE: usize = 100;

#[derive(Clone)]
pub struct DeploymentManager {
    pipelines: Vec<Pipeline>,
}

impl DeploymentManager {
    /// Create a new deployment manager. Manages one or more 'pipelines' for
    /// processing service building, loading, and deployment.
    pub fn new(persistence: Persistence, pipeline_count: usize) -> Self {
        DeploymentManager {
            pipelines: (0..pipeline_count)
                .into_iter()
                .map(|i| Pipeline::new(i, persistence.clone()))
                .collect(),
        }
    }

    pub async fn queue_push(&self, queued: Queued) {
        let highest_capacity = self
            .pipelines
            .iter()
            .max_by(|x, y| x.queue_send.capacity().cmp(&y.queue_send.capacity()))
            .expect("Deployment manager has no pipelines");

        highest_capacity.queue_send.send(queued).await.unwrap();
    }

    pub async fn run_push(&self, built: Built) {
        let highest_capacity = self
            .pipelines
            .iter()
            .max_by(|x, y| x.run_send.capacity().cmp(&y.run_send.capacity()))
            .expect("Deployment manager has no pipelines");

        highest_capacity.run_send.send(built).await.unwrap();
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
    queue_task: Arc<JoinHandle<()>>,
    run_task: Arc<JoinHandle<()>>,
}

impl Pipeline {
    /// Creates two Tokio tasks, one for building queued services, the other for
    /// executing/deploying built services. Two multi-producer, single consumer
    /// channels are also created which are for moving on-going service
    /// deployments between the aforementioned tasks.
    fn new(ident: usize, persistence: Persistence) -> Pipeline {
        let (queue_send, queue_recv) = mpsc::channel(QUEUE_BUFFER_SIZE);
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);

        let run_send_clone = run_send.clone();
        let persistence_clone = persistence.clone();

        let queue_task = tokio::spawn(async move {
            queue::task(ident, queue_recv, run_send_clone, persistence).await
        });
        let run_task =
            tokio::spawn(async move { run::task(ident, run_recv, persistence_clone).await });

        Pipeline {
            queue_send,
            run_send,
            queue_task: Arc::new(queue_task),
            run_task: Arc::new(run_task),
        }
    }
}

type QueueSender = mpsc::Sender<queue::Queued>;
type QueueReceiver = mpsc::Receiver<queue::Queued>;

type RunSender = mpsc::Sender<run::Built>;
type RunReceiver = mpsc::Receiver<run::Built>;
