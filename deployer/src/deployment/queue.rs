use axum::body::Bytes;
use futures::{Stream, StreamExt};
use tracing::{debug, error, info, instrument};

use super::{Built, QueueReceiver, RunSender};
use crate::deployment::State;
use crate::error::Result;

use std::fmt;
use std::pin::Pin;

pub async fn task(mut recv: QueueReceiver, run_send: RunSender) {
    info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        let name = queued.name.clone();

        info!("Queued deployment at the front of the queue: {}", name);

        let run_send_cloned = run_send.clone();

        tokio::spawn(async move {
            match queued.handle().await {
                Ok(built) => promote_to_run(built, run_send_cloned).await,
                Err(e) => error!("Error during building of deployment '{}' - {e}", name),
            }
        });
    }
}

#[instrument(fields(name = built.name.as_str(), state = %State::Built))]
async fn promote_to_run(built: Built, run_send: RunSender) {
    run_send.send(built).await.unwrap();
}

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
}

impl Queued {
    #[instrument(skip(self), fields(name = self.name.as_str(), state = %State::Building))]
    async fn handle(mut self) -> Result<Built> {
        // Read POSTed data:

        while let Some(chunk) = self.data_stream.next().await {
            let chunk = chunk?;
            debug!("{} - streamed {} bytes", self.name, chunk.len());
        }

        // Build:

        // TODO

        // Update deployment state to 'built:

        let built = Built { name: self.name };

        Ok(built)
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Queued {{ name: \"{}\", .. }}", self.name)
    }
}
