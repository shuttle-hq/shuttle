use std::path::PathBuf;

use tracing::{debug, error, info, instrument};

use super::{KillReceiver, KillSender, RunReceiver, State};
use crate::error::Result;

pub async fn task(mut recv: RunReceiver, kill_send: KillSender) {
    info!("Run task started");

    while let Some(built) = recv.recv().await {
        let name = built.name.clone();

        info!("Built deployment at the front of run queue: {}", name);

        let kill_recv = kill_send.subscribe();

        tokio::spawn(async move {
            if let Err(e) = built.handle(kill_recv, |_built| {}).await {
                error!("Error during running of deployment '{}' - {e}", name);
            }
        });
    }
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub so_path: PathBuf,
}

impl Built {
    #[instrument(skip(self, handle_cleanup), fields(name = self.name.as_str(), state = %State::Running))]
    async fn handle(
        self,
        mut kill_recv: KillReceiver,
        handle_cleanup: impl FnOnce(Built) + Send + 'static,
    ) -> Result<()> {
        // Load service into memory:
        // TODO
        let mut execute_future = Box::pin(async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }); // placeholder

        // Execute loaded service:

        tokio::spawn(async move {
            tokio::select! {
                Ok(name) = kill_recv.recv() => {
                    if name == self.name {
                        debug!("Service {name} killed");
                        // execute_future.abort();
                    }
                }
                _ = &mut execute_future => {}
            }

            handle_cleanup(self);
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};

    use tokio::sync::{broadcast, oneshot};

    use super::Built;

    #[tokio::test]
    async fn can_be_killed() {
        let built = Built {
            name: "test".to_string(),
            so_path: PathBuf::new(),
        };
        let (kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |_built| {
            cleanup_send.send(()).unwrap();
        };

        built.handle(kill_recv, handle_cleanup).await.unwrap();

        kill_send.send("test".to_string()).unwrap();

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => panic!("cleanup should be called"),
            _ = cleanup_recv => {}
        }
    }
}
