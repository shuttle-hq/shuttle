use axum::body::Bytes;
use futures::{Stream, StreamExt};

use super::{Built, QueueReceiver, RunSender};
use crate::deployment::DeploymentState;
use crate::error::Result;
use crate::persistence::Persistence;

use std::fmt;
use std::pin::Pin;

pub async fn task(
    ident: usize,
    mut recv: QueueReceiver,
    run_send: RunSender,
    persistence: Persistence,
) {
    log::info!("Queue task {ident} started");

    while let Some(queued) = recv.recv().await {
        log::info!(
            "Queued deployment at the front of the queue {ident}: {}",
            queued.name
        );

        tokio::spawn(queued.handle(run_send.clone(), persistence.clone()));
    }
}

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
    pub state: DeploymentState,
}

impl Queued {
    async fn handle(mut self, run_send: RunSender, persistence: Persistence) {
        // Update deployment state:

        self.state = DeploymentState::Building;

        persistence.update_deployment(&self).await.expect("TODO");

        // Read POSTed data:

        while let Some(chunk) = self.data_stream.next().await {
            let chunk = chunk.expect("TODO");
            log::debug!("{} - streamed {} bytes", self.name, chunk.len());
        }

        // Build:

        // TODO

        // Update deployment state to 'built:

        let built = Built {
            name: self.name,
            state: DeploymentState::Built,
        };

        persistence.update_deployment(&built).await.expect("TODO");

        // Send to run queue:

        run_send.send(built).await.unwrap();
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
