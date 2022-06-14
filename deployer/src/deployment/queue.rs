use super::{QueueReceiver, ServiceID};

pub async fn task(recv: QueueReceiver) {
    log::info!("Queue task started");
}

#[derive(Debug)]
pub struct Queued {
    pub id: ServiceID,
}
