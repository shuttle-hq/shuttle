use super::{Built, QueueReceiver, RunSender};
use crate::deployment::DeploymentState;
use crate::persistence::Persistence;

use std::fmt;
use std::future::Future;
use std::pin::Pin;

pub async fn task(
    ident: usize,
    mut recv: QueueReceiver,
    run_send: RunSender,
    persistence: Persistence,
) {
    log::info!("Queue task {ident} started");

    while let Some(mut queued) = recv.recv().await {
        log::info!(
            "Queued deployment at the front of the queue {ident}: {}",
            queued.name
        );

        // Update deployment state:

        queued.state = DeploymentState::Building;

        persistence.update_deployment(&queued).await.expect("TODO");

        // Read POSTed data:

        let data = queued
            .data_future
            .await
            .expect("TODO: Enter DeploymentState::Error instead of panicing");

        log::debug!("{} - received {} bytes", queued.name, data.len());

        // Build:

        // TODO

        // Update deployment state to 'built:

        let built = Built {
            name: queued.name,
            state: DeploymentState::Built,
        };

        persistence.update_deployment(&built).await.expect("TODO");

        // Send to run queue:

        run_send.send(built).await.unwrap();
    }
}

pub struct Queued {
    pub name: String,
    pub data_future: Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send + Sync>>,
    pub state: DeploymentState,
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
