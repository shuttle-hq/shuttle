mod states;
mod queue;
mod run;

use tokio::sync::mpsc;

const QUEUE_BUFFER_SIZE: usize = 100;
const RUN_BUFFER_SIZE: usize = 100;

#[derive(Clone)]
pub struct DeploymentManager {
    queue_send: QueueSender,
    run_send: RunSender,
}

impl DeploymentManager {
    pub fn new() -> Self {
        let (queue_send, queue_recv) = mpsc::channel(QUEUE_BUFFER_SIZE);
        let (run_send, run_recv) = mpsc::channel(RUN_BUFFER_SIZE);

        tokio::spawn(async move { queue::task(queue_recv).await });
        tokio::spawn(async move { run::task(run_recv).await });

        DeploymentManager {
            queue_send,
            run_send,
        }
    }
}

type QueueSender = mpsc::Sender<()>;
type QueueReceiver = mpsc::Receiver<()>;

type RunSender = mpsc::Sender<()>;
type RunReceiver = mpsc::Receiver<()>;
