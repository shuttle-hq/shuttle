mod build_logs;
pub mod deploy_layer;
mod info;
pub mod log;
mod queue;
mod run;
mod states;

pub use info::DeploymentInfo;
pub use states::State;

pub use log::Log;
pub use queue::Queued;
pub use run::Built;

use build_logs::{BuildLogWriter, BuildLogsManager};

use crate::error::Result;

use bytes::Bytes;
use futures::Stream;
use tokio::sync::{broadcast, mpsc};
use tracing::instrument;

const QUEUE_BUFFER_SIZE: usize = 100;
const RUN_BUFFER_SIZE: usize = 100;
const KILL_BUFFER_SIZE: usize = 10;

#[derive(Clone)]
pub struct DeploymentManager {
    pipeline: Pipeline,
    kill_send: KillSender,
    build_logs: BuildLogsManager,
}

impl DeploymentManager {
    /// Create a new deployment manager. Manages one or more 'pipelines' for
    /// processing service building, loading, and deployment.
    pub fn new() -> Self {
        let (kill_send, _) = broadcast::channel(KILL_BUFFER_SIZE);

        DeploymentManager {
            pipeline: Pipeline::new(kill_send.clone()),
            kill_send,
            build_logs: BuildLogsManager::new(),
        }
    }

    #[instrument(skip(self, data_stream), fields(name = name.as_str(), state = %State::Queued))]
    pub async fn queue_push(
        &self,
        name: String,
        data_stream: impl Stream<Item = Result<Bytes>> + Send + Sync + 'static,
    ) -> DeploymentInfo {
        let build_log_writer = self.build_logs.for_deployment(name.clone()).await;

        let queued = Queued {
            name,
            data_stream: Box::pin(data_stream),
            build_log_writer,
        };
        let info = DeploymentInfo::from(&queued);

        self.pipeline.queue_send.send(queued).await.unwrap();

        info
    }

    #[instrument(skip(self), fields(name = name.as_str(), state = %State::Built))]
    pub async fn run_push(&self, name: String) -> DeploymentInfo {
        let built = Built { name };
        let info = DeploymentInfo::from(&built);

        self.pipeline.run_send.send(built).await.unwrap();

        info
    }

    pub async fn kill(&self, name: String) {
        self.build_logs.delete_deployment(&name).await;

        if self.kill_send.receiver_count() > 0 {
            self.kill_send.send(name).unwrap();
        }
    }

    pub async fn build_logs_subscribe(&self, name: &str) -> Option<BuildLogReceiver> {
        self.build_logs.subscribe(name).await
    }

    pub async fn build_logs_so_far(&self, name: &str) -> Option<Vec<String>> {
        self.build_logs.get_logs_so_far(name).await
    }
}

/// ```
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
    fn new(kill_send: KillSender) -> Pipeline {
        let (queue_send, queue_recv) = mpsc::channel(QUEUE_BUFFER_SIZE);
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);

        let run_send_clone = run_send.clone();

        tokio::spawn(async move { queue::task(queue_recv, run_send_clone).await });
        tokio::spawn(async move { run::task(run_recv, kill_send).await });

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

type BuildLogSender = broadcast::Sender<String>;
pub type BuildLogReceiver = broadcast::Receiver<String>;
