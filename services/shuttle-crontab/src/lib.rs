use std::str::FromStr;
use std::sync::Arc;

use axum::Router;
use chrono::Utc;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use shuttle_persist::PersistInstance;
use shuttle_runtime::tracing::info;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::sleep,
};

mod router;

pub type ShuttleCrontab = Result<CrontabService, shuttle_runtime::Error>;

#[derive(Debug, Serialize, Deserialize)]
pub struct RawJob {
    schedule: String,
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Crontab {
    jobs: Vec<RawJob>,
}

#[derive(Debug)]
pub struct CronJob {
    schedule: Schedule,
    url: String,
}

impl CronJob {
    fn from_raw(raw: &RawJob) -> Self {
        let schedule = Schedule::from_str(&raw.schedule).expect("Failed to parse schedule");
        Self {
            schedule,
            url: raw.url.clone(),
        }
    }

    async fn run(&self) {
        info!("Running job for: {}", self.url);
        while let Some(next_run) = self.schedule.upcoming(Utc).next() {
            let next_run_in = next_run
                .signed_duration_since(chrono::offset::Utc::now())
                .to_std()
                .unwrap();
            sleep(next_run_in).await;

            info!("Calling {}", self.url);

            let req = reqwest::get(self.url.clone()).await.unwrap();
            info!("Called with status code {}", req.status());
        }
    }
}

pub struct CronRunner {
    persist: PersistInstance,
    receiver: Receiver<RawJob>,
}

impl CronRunner {
    async fn run_jobs(&mut self) {
        if let Ok(tab) = self.persist.load::<Crontab>("crontab") {
            info!("Found {} jobs", tab.jobs.len());
            for raw in tab.jobs {
                info!("Starting job: {:?}", raw);
                let job = CronJob::from_raw(&raw);

                tokio::spawn(async move {
                    job.run().await;
                });
            }
        }

        while let Some(raw) = self.receiver.recv().await {
            info!("Channel received: {:?}", raw);
            let mut crontab = match self.persist.load::<Crontab>("crontab") {
                Ok(tab) => tab,
                Err(_) => Crontab { jobs: vec![] },
            };

            let job = CronJob::from_raw(&raw);

            crontab.jobs.push(raw);

            info!("Persisting: {:?}", crontab);
            self.persist.save("crontab", crontab).unwrap();

            // Spawn new job
            tokio::spawn(async move {
                job.run().await;
            });
        }
    }
}

pub struct CrontabService {
    router: Router,
    runner: CronRunner,
}

impl CrontabService {
    pub fn new(persist: PersistInstance) -> Result<CrontabService, shuttle_runtime::Error> {
        let (sender, receiver) = mpsc::channel(32);
        let runner = CronRunner { persist, receiver };
        let state = Arc::new(AppState { sender });
        let router = router::build_router(state);

        Ok(Self { router, runner })
    }
}

pub struct AppState {
    sender: Sender<RawJob>,
}

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for CrontabService {
    async fn bind(mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        let router = self.router;
        let mut runner = self.runner;

        let server = axum::Server::bind(&addr);

        let (_runner_hdl, _axum_hdl) =
            tokio::join!(runner.run_jobs(), server.serve(router.into_make_service()));

        Ok(())
    }
}
