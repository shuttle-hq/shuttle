use std::collections::HashMap;
use std::sync::Arc;

use shuttle_backends::project_name::ProjectName;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::task::{BoxedTask, TaskResult};
use crate::Error;

pub const WORKER_QUEUE_SIZE: usize = 2048;

pub struct Worker {
    send: Option<Sender<BoxedTask>>,
    recv: Receiver<BoxedTask>,
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

impl Worker {
    pub fn new() -> Self {
        let (send, recv) = channel(WORKER_QUEUE_SIZE);
        Self {
            send: Some(send),
            recv,
        }
    }

    /// Returns a [Sender] to push work to this worker.
    ///
    /// # Panics
    /// If this worker has already started.
    pub fn sender(&self) -> Sender<BoxedTask> {
        Sender::clone(self.send.as_ref().unwrap())
    }

    /// Starts the worker, waiting and processing elements from the
    /// queue until the last sending end for the channel is dropped,
    /// at which point this future resolves.
    ///
    /// # Panics
    /// If this worker has already started.
    pub async fn start(mut self) -> Result<Self, Error> {
        // Drop the self-sender owned by this worker to prevent a
        // deadlock if all the other senders have already been dropped
        // at this point.
        let _ = self.send.take().unwrap();
        debug!("starting worker");

        while let Some(mut work) = self.recv.recv().await {
            loop {
                match work.poll(()).await {
                    TaskResult::Done(_) | TaskResult::Cancelled => break,
                    TaskResult::Pending(_) | TaskResult::TryAgain => continue,
                    TaskResult::Err(err) => {
                        warn!("task failed: {err}");
                        break;
                    }
                }
            }
        }

        Ok(self)
    }
}

#[derive(Clone, Default)]
pub struct TaskRouter {
    table: Arc<RwLock<HashMap<ProjectName, Sender<BoxedTask>>>>,
}

impl TaskRouter {
    pub async fn route(
        &self,
        name: &ProjectName,
        task: BoxedTask,
    ) -> Result<(), SendError<BoxedTask>> {
        let mut table = self.table.write().await;
        if let Some(sender) = table.get(name) {
            sender.send(task).await
        } else {
            let worker = Worker::new();
            let sender = worker.sender();

            tokio::spawn(worker.start());

            let res = sender.send(task).await;

            table.insert(name.clone(), sender);

            res
        }
    }
}
