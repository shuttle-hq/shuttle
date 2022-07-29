use std::{thread::sleep, time::Duration};

use shuttle_service::Service;

struct SleepService {
    duration: u64,
}

#[shuttle_service::main]
async fn simple() -> Result<SleepService, shuttle_service::Error> {
    Ok(SleepService { duration: 10 })
}

#[shuttle_service::async_trait]
impl Service for SleepService {
    async fn bind(
        mut self: Box<Self>,
        _: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        let duration = Duration::from_secs(self.duration);

        sleep(duration);
        Ok(())
    }
}
