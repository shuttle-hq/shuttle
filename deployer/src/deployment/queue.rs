use super::{Built, QueueReceiver, RunSender};
use crate::deployment::DeploymentState;
use crate::error::Result;
use crate::persistence::Persistence;

use shuttle_service::loader::build_crate;

use std::fmt;
use std::path::{PathBuf, Path};
use std::pin::Pin;
use std::io::Read;

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

        extract_tar_gz_data(vec.as_slice(), &project_path)?;

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

/// Equivalent to the command: `tar -xzf --strip-components 1`
fn extract_tar_gz_data(data: impl Read, dest: impl AsRef<Path>) -> Result<()> {
    let tar = GzDecoder::new(data);
    let mut archive = Archive::new(tar);
    archive.set_overwrite(true);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path: PathBuf = entry.path()?.components().skip(1).collect();
        entry.unpack(dest.as_ref().join(path))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tar_gz_extraction() {
        // Binary data for an archive in the following form:
        //
        // - temp
        //   - world.txt
        //   - subdir
        //     - hello.txt
        let test_data = hex::decode("\
1f8b0800000000000003edd5d10a823014c6f15df7143e41ede8997b1e4d\
a3c03074528f9f0a41755174b1a2faff6e0653d8818f7d0bf5feb03271d9\
91f76e5ac53b7bbd5e18d1d4a96a96e6a9b16225f7267191e79a0d7d28ba\
2431fbe2f4f0bf67dfbf5498f23fb65d532dc329c439630a38cff541fe7a\
977f6a9d98c4c619e7d69fe75f94ebc5a767c0e7ccf7bf1fca6ad7457b06\
5eea7f95f1fe8b3aa5ffdfe13aff6ddd346d8467e0a5fef7e3be649928fd\
ff0e55bda1ff01000000000000000000e0079c01ff12a55500280000").unwrap();

        extract_tar_gz_data(test_data.as_slice(), "/tmp/shuttle-extraction-test").unwrap();
        assert!(fs::read_to_string("/tmp/shuttle-extraction-test/world.txt").await.unwrap().starts_with("abc"));
        assert!(fs::read_to_string("/tmp/shuttle-extraction-test/subdir/hello.txt").await.unwrap().starts_with("def"));
    }
}
