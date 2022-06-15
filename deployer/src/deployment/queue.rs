use super::QueueReceiver;
use crate::deployment::DeploymentState;
use crate::persistence::Persistence;

use std::fmt;
use std::future::Future;
use std::pin::Pin;

pub async fn task(mut recv: QueueReceiver, persistence: Persistence) {
    log::info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        log::info!(
            "Queued deployment received the front of the queue: {}",
            queued.name
        );

        persistence
            .deployment((&queued).into())
            .await
            .expect("TODO");

        let data = queued
            .data_future
            .await
            .expect("TODO: Enter DeploymentState::Error instead of panicing");
        log::debug!("{} - received {} bytes", queued.name, data.len());
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
