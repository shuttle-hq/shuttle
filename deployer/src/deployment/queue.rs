use super::QueueReceiver;

pub async fn task(recv: QueueReceiver) {
    log::info!("Queue task started");
}
