use std::time::Duration;

use shuttle_runtime::Service;
use tokio::time::sleep;

struct SleepService {
    duration: u64,
}

#[shuttle_runtime::main]
async fn simple() -> Result<SleepService, shuttle_runtime::Error> {
    Ok(SleepService { duration: 4 })
}

#[shuttle_runtime::async_trait]
impl Service for SleepService {
    async fn bind(mut self, _: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        let duration = Duration::from_secs(self.duration);

        sleep(duration).await;
        Ok(())
    }
}
