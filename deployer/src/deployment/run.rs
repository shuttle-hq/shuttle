use super::{DeploymentState, RunReceiver};

pub async fn task(recv: RunReceiver) {
    log::info!("Run task started");
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub state: DeploymentState,
}
