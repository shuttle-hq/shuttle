use super::QueueReceiver;

use std::fmt;
use std::future::Future;
use std::pin::Pin;

pub async fn task(mut recv: QueueReceiver) {
    log::info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        log::info!(
            "Queued deployment received the front of the queue: {}",
            queued.name
        );

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
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Queued {{ name: \"{}\", .. }}", self.name)
    }
}
