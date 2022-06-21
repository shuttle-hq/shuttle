use axum::body::Bytes;
use futures::{Stream, StreamExt};

use super::{Built, QueueReceiver, RunSender};
use crate::deployment::DeploymentState;
use crate::error::Result;
use crate::persistence::Persistence;

use std::fmt;
use std::pin::Pin;

pub async fn task(mut recv: QueueReceiver, run_send: RunSender, persistence: Persistence) {
    log::info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        let name = queued.name.clone();

        log::info!("Queued deployment at the front of the queue: {}", name);

        let run_send_cloned = run_send.clone();
        let persistence_cloned = persistence.clone();

        tokio::spawn(async move {
            if let Err(e) = queued.handle(run_send_cloned, persistence_cloned).await {
                log::error!("Error during building of deployment '{}' - {e}", name);
            }
        });
    }
}

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
    pub state: DeploymentState,
}

impl Queued {
    async fn handle(mut self, run_send: RunSender, persistence: Persistence) -> Result<()> {
        // Update deployment state:

        self.state = DeploymentState::Building;

        persistence.update_deployment(&self).await?;

        // Read POSTed data:

        while let Some(chunk) = self.data_stream.next().await {
            let chunk = chunk?;
            log::debug!("{} - streamed {} bytes", self.name, chunk.len());
        }

        // Build:

        // TODO

        // Update deployment state to 'built:

        let built = Built {
            name: self.name,
            state: DeploymentState::Built,
        };

        persistence.update_deployment(&built).await?;

        // Send to run queue:

        run_send.send(built).await.unwrap();

        Ok(())
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Queued {{ name: \"{}\", state: {}, .. }}",
            self.name, self.state
        )
    }
}
