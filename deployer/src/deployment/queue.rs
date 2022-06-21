use super::{Built, QueueReceiver, RunSender};
use crate::deployment::DeploymentState;
use crate::error::Result;
use crate::persistence::Persistence;

use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;

use bytes::{BufMut, Bytes};
use flate2::read::GzDecoder;
use futures::{Stream, StreamExt};
use tar::Archive;
use tokio::fs;

const BUILDS_PATH: &str = "shuttle-builds";

pub async fn task(mut recv: QueueReceiver, run_send: RunSender, persistence: Persistence) {
    log::info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        let name = queued.name.clone();

        log::info!("Queued deployment at the front of the queue: {}", name);

        let run_send_cloned = run_send.clone();
        let persistence_cloned = persistence.clone();

        tokio::spawn(async move {
            if let Err(e) = queued.handle(run_send_cloned, persistence_cloned).await {
                log::error!("Error during building of deployment '{}' - {e}", name);
            }
        });
    }
}

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
    pub state: DeploymentState,
}

impl Queued {
    async fn handle(mut self, run_send: RunSender, persistence: Persistence) -> Result<()> {
        // Update deployment state:

        self.state = DeploymentState::Building;

        persistence.update_deployment(&self).await?;

        // Read POSTed data:

        let mut vec = Vec::with_capacity(self.data_stream.size_hint().0);
        while let Some(buf) = self.data_stream.next().await {
            let buf = buf?;
            log::debug!("Received {} bytes for deployment {}", buf.len(), self.name);
            vec.put(buf);
        }

        // Extract .tar.gz data:

        fs::create_dir_all(BUILDS_PATH).await?;

        let archive_path = PathBuf::from(BUILDS_PATH).join(&self.name);

        let tar = GzDecoder::new(vec.as_slice());
        let mut archive = Archive::new(tar);
        archive.unpack(archive_path)?;

        // Build:

        // TODO

        // Update deployment state to built:

        let built = Built {
            name: self.name,
            state: DeploymentState::Built,
        };

        persistence.update_deployment(&built).await?;

        // Send to run queue:

        run_send.send(built).await.unwrap();

        Ok(())
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Queued {{ name: \"{}\", state: {}, .. }}",
            self.name, self.state
        )
    }
}
