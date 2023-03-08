use std::time::Duration;

use shuttle_service::Service;
use tokio::time::sleep;

struct SleepService {
    duration: u64,
}

#[shuttle_service::main]
async fn simple() -> Result<SleepService, shuttle_service::Error> {
    Ok(SleepService { duration: 4 })
}

#[shuttle_service::async_trait]
impl Service for SleepService {
    async fn bind(mut self, _: std::net::SocketAddr) -> Result<(), shuttle_service::error::Error> {
        let duration = Duration::from_secs(self.duration);

        sleep(duration).await;
        Ok(())
    }
}
