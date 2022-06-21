use super::{Built, QueueReceiver, RunSender};
use crate::deployment::DeploymentState;
use crate::error::Result;
use crate::persistence::Persistence;

use shuttle_service::loader::build_crate;

use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;

use bytes::{BufMut, Bytes};
use flate2::read::GzDecoder;
use futures::{Stream, StreamExt};
use rand::distributions::DistString;
use tar::Archive;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Path of the directory that contains extracted service Cargo projects.
const BUILDS_PATH: &str = "shuttle-builds";

/// The name given to 'marker files' (text files placed in project directories
/// that have in them the name of the linked library '.so' file of that service
/// when built.
const MARKER_FILE_NAME: &str = ".shuttle-marker";

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

// TODO:
// * Handling code shared between services? Git dependencies?
// * Ensure builds do not interfere with one another.

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

        // Extract '.tar.gz' data:

        fs::create_dir_all(BUILDS_PATH).await?;

        let project_path = PathBuf::from(BUILDS_PATH).join(&self.name);

        let tar = GzDecoder::new(vec.as_slice());
        let mut archive = Archive::new(tar);
        archive.unpack(&project_path)?;

        // Build:

        let cargo_output_buf = Box::new(std::io::stdout());

        let so_path = build_crate(&project_path, cargo_output_buf).unwrap(); // TODO: Handle error

        // Remove old build if any:

        let marker_path = project_path.join(MARKER_FILE_NAME);

        if let Ok(mut existing_marker_file) = fs::File::open(&marker_path).await {
            let mut old_so_name = String::new();
            existing_marker_file
                .read_to_string(&mut old_so_name)
                .await?;

            let old_so_path = project_path.join(old_so_name);

            if old_so_path.exists() {
                fs::remove_file(old_so_path).await?;
            }
        }

        // Copy and rename the built '.so' file:

        let random_so_name =
            rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
        let random_so_path = project_path.join(&random_so_name);
        fs::copy(so_path, random_so_path).await?;

        // Create a marker file to indicate the name of the '.so' file:

        fs::write(marker_path, random_so_name).await?;

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
