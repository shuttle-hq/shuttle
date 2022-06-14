use super::QueueReceiver;

pub async fn task(mut recv: QueueReceiver) {
    log::info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        log::info!(
            "Queued deployment received the front of the queue: {}",
            queued.name
        );
    }
}

#[derive(Debug)]
pub struct Queued {
    pub name: String,
}
