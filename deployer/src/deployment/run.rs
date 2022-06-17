use super::{DeploymentState, KillReceiver, KillSender, RunReceiver};
use crate::persistence::Persistence;

pub async fn task(
    ident: usize,
    mut recv: RunReceiver,
    kill_send: KillSender,
    persistence: Persistence,
) {
    log::info!("Run task {ident} started");

    while let Some(built) = recv.recv().await {
        log::info!(
            "Built deployment at the front of run queue {ident}: {}",
            built.name
        );

        tokio::spawn(built.handle(kill_send.subscribe(), persistence.clone()));
    }
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub state: DeploymentState,
}

impl Built {
    async fn handle(mut self, mut kill_recv: KillReceiver, persistence: Persistence) {
        // Load service into memory:
        // TODO
        let mut execute_future = Box::pin(async { loop {} }); // placeholder

        // Update deployment state:

        self.state = DeploymentState::Running;

        persistence
            .update_deployment(&self)
            .await
            .unwrap_or_else(|e| log::error!("{}", e));

        // Execute loaded service:

        loop {
            tokio::select! {
                Ok(n) = kill_recv.recv() => {
                    if n == self.name {
                        log::debug!("Service {n} killed");
                        break;
                    }
                }
                _ = &mut execute_future => {}
            }
        }
    }
}
