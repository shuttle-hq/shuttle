use chrono::Utc;
use std::future::Future;
use tokio::time::sleep;

use cron::Schedule;
use shuttle_runtime::{async_trait, Service};

pub struct CronService<F> {
    pub schedule: Schedule,
    pub job: fn() -> F,
}

impl<F: Future> CronService<F> {
    async fn start(&self) {
        while let Some(next_run) = self.schedule.upcoming(Utc).next() {
            let next_run_in = next_run
                .signed_duration_since(chrono::offset::Utc::now())
                .to_std()
                .unwrap();
            sleep(next_run_in).await;
            (self.job)().await;
        }
    }
}

#[async_trait]
impl<F> Service for CronService<F>
where
    F: Future + Send + Sync + 'static,
{
    async fn bind(
        mut self,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        self.start().await;

        Ok(())
    }
}
