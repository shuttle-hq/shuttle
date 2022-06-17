use bytes::Bytes;
use futures::{Stream, StreamExt};

use super::{Built, QueueReceiver, RunSender};
use crate::deployment::{DeploymentInfo, DeploymentState};
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

    while let Some(queued) = recv.recv().await {
        log::info!(
            "Queued deployment at the front of the queue {ident}: {}",
            queued.name
        );

        let mut info = DeploymentInfo::from(&queued);

        if let Err(_e) = handle_queued(queued, &persistence, &run_send).await {
            // TODO: Do something with error msg.

            info.state = DeploymentState::Error;
            persistence
                .update_deployment(info)
                .await
                .unwrap_or_else(|db_err| log::error!("{}", db_err));
        }
    }
}

async fn handle_queued(
    mut queued: Queued,
    persistence: &Persistence,
    run_send: &RunSender,
) -> anyhow::Result<()> {
    // Update deployment state:

    queued.state = DeploymentState::Building;

    persistence.update_deployment(&queued).await?;

    // Read POSTed data:

    while let Some(chunk) = queued.data_stream.next().await {
        let chunk = chunk?;
        log::debug!("{} - streamed {} bytes", queued.name, chunk.len());
    }

    // Build:

    // TODO

    // Update deployment state to 'built:

    let built = Built {
        name: queued.name,
        state: DeploymentState::Built,
    };

    persistence.update_deployment(&built).await?;

    // Send to run queue:

    run_send.send(built).await.unwrap();

    Ok(())
}

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes, axum::Error>> + Send + Sync>>,
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
