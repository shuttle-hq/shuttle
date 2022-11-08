use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{debug, info};

use crate::task::{BoxedTask, TaskResult};
use crate::Error;

pub const WORKER_QUEUE_SIZE: usize = 2048;

pub struct Worker<W = BoxedTask> {
    send: Option<Sender<W>>,
    recv: Receiver<W>,
}

impl<W> Default for Worker<W>
where
    W: Send,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<W> Worker<W>
where
    W: Send,
{
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
    pub fn sender(&self) -> Sender<W> {
        Sender::clone(self.send.as_ref().unwrap())
    }
}

impl Worker<BoxedTask> {
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
                        info!("task failed: {err}");
                        break;
                    }
                }
            }
        }

        Ok(self)
    }
}
