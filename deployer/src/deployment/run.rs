use super::{DeploymentState, KillReceiver, KillSender, RunReceiver};
use crate::error::Result;
use crate::persistence::Persistence;

pub async fn task(mut recv: RunReceiver, kill_send: KillSender, persistence: Persistence) {
    log::info!("Run task started");

    while let Some(built) = recv.recv().await {
        let name = built.name.clone();

        log::info!("Built deployment at the front of run queue: {}", name);

        let kill_recv = kill_send.subscribe();
        let persistence_cloned = persistence.clone();

        tokio::spawn(async move {
            if let Err(e) = built.handle(kill_recv, persistence_cloned).await {
                log::error!("Error during running of deployment '{}' - {e}", name);
            }
        });
    }
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub state: DeploymentState,
}

impl Built {
    async fn handle(mut self, mut kill_recv: KillReceiver, persistence: Persistence) -> Result<()> {
        // Load service into memory:
        // TODO
        let mut execute_future = Box::pin(async { loop {} }); // placeholder

        // Update deployment state:

        self.state = DeploymentState::Running;

        persistence.update_deployment(&self).await?;

        // Execute loaded service:

        loop {
            tokio::select! {
                Ok(name) = kill_recv.recv() => {
                    if name == self.name {
                        log::debug!("Service {name} killed");
                        break;
                    }
                }
                _ = &mut execute_future => {}
            }
        }

        Ok(())
    }
}
