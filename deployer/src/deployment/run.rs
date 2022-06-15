use super::{DeploymentState, RunReceiver};
use crate::persistence::Persistence;

pub async fn task(ident: usize, mut recv: RunReceiver, persistence: Persistence) {
    log::info!("Run task {ident} started");

    while let Some(mut built) = recv.recv().await {
        log::info!(
            "Built deployment at the front of run queue {ident}: {}",
            built.name
        );

        // Load service into memory:

        // TODO

        // Execute loaded service:

        // TODO

        // Update deployment state:

        built.state = DeploymentState::Running;

        persistence
            .update_deployment(&built)
            .await
            .expect("TODO: handle");
    }
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub state: DeploymentState,
}
