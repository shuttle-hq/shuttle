use super::{BuildLogWriter, Built, DeploymentState, QueueReceiver, RunSender};
use crate::error::{Error, Result};
use crate::persistence::Persistence;

use shuttle_service::loader::build_crate;

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use bytes::{BufMut, Bytes};
use flate2::read::GzDecoder;
use futures::{Stream, StreamExt};
use rand::distributions::DistString;
use tar::Archive;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Path of the directory that contains extracted service Cargo projects.
const BUILDS_PATH: &str = "/tmp/shuttle-builds";

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

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
    pub state: DeploymentState,
    pub build_log_writer: BuildLogWriter,
}

impl Queued {
    async fn handle(mut self, run_send: RunSender, persistence: Persistence) -> Result<()> {
        // Update deployment state:

        self.state = DeploymentState::Building;

        persistence.update_deployment(&self).await?;

        log::info!("Fetching POSTed data for deployment '{}'", self.name);

        let mut vec = Vec::new();
        while let Some(buf) = self.data_stream.next().await {
            let buf = buf?;
            log::debug!("Received {} bytes for deployment {}", buf.len(), self.name);
            vec.put(buf);
        }

        log::info!("Extracting received data for deployment '{}'", self.name);

        fs::create_dir_all(BUILDS_PATH).await?;

        let project_path = PathBuf::from(BUILDS_PATH).join(&self.name);

        extract_tar_gz_data(vec.as_slice(), &project_path)?;

        log::info!("Building deployment '{}'", self.name);

        let project_path = project_path.canonicalize()?;
        let so_path = build_crate(&project_path, Box::new(self.build_log_writer))
            .map_err(|e| Error::Build(e.into()))?;

        log::info!("Removing old build (if present) for {}", self.name);

        remove_old_build(&project_path).await?;

        log::info!(
            "Moving built library and creating marker file for deployment '{}'",
            self.name
        );

        rename_build(&project_path, so_path).await?;

        // Update deployment state to built:

        let built = Built {
            name: self.name,
            state: DeploymentState::Built,
        };

        persistence.update_deployment(&built).await?;

        log::info!("Moving deployment '{}' to run queue", built.name);

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

/// Check for an existing marker file is the specified project directory and
/// if one exists delete the indicated '.so' file.
async fn remove_old_build(project_path: impl AsRef<Path>) -> Result<()> {
    let marker_path = project_path.as_ref().join(MARKER_FILE_NAME);

    if let Ok(mut existing_marker_file) = fs::File::open(&marker_path).await {
        let mut old_so_name = String::new();
        existing_marker_file
            .read_to_string(&mut old_so_name)
            .await?;

        let old_so_path = project_path.as_ref().join(old_so_name);

        if old_so_path.exists() {
            fs::remove_file(old_so_path).await?;
        }

        fs::remove_file(marker_path).await?;
    }

    Ok(())
}

async fn rename_build(project_path: impl AsRef<Path>, so_path: impl AsRef<Path>) -> Result<()> {
    let random_so_name =
        rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let random_so_path = project_path.as_ref().join(&random_so_name);

    fs::rename(so_path, random_so_path).await?;

    fs::write(project_path.as_ref().join(MARKER_FILE_NAME), random_so_name).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use tokio::fs;

    use crate::deployment::queue::MARKER_FILE_NAME;

    #[tokio::test]
    async fn extract_tar_gz_data() {
        let p = Path::new("/tmp/shuttle-extraction-test");

        // Binary data for an archive in the following form:
        //
        // - temp
        //   - world.txt
        //   - subdir
        //     - hello.txt
        let test_data = hex::decode(
            "\
1f8b0800000000000003edd5d10a823014c6f15df7143e41ede8997b1e4d\
a3c03074528f9f0a41755174b1a2faff6e0653d8818f7d0bf5feb03271d9\
91f76e5ac53b7bbd5e18d1d4a96a96e6a9b16225f7267191e79a0d7d28ba\
2431fbe2f4f0bf67dfbf5498f23fb65d532dc329c439630a38cff541fe7a\
977f6a9d98c4c619e7d69fe75f94ebc5a767c0e7ccf7bf1fca6ad7457b06\
5eea7f95f1fe8b3aa5ffdfe13aff6ddd346d8467e0a5fef7e3be649928fd\
ff0e55bda1ff01000000000000000000e0079c01ff12a55500280000",
        )
        .unwrap();

        super::extract_tar_gz_data(test_data.as_slice(), &p).unwrap();
        assert!(fs::read_to_string(p.join("world.txt"))
            .await
            .unwrap()
            .starts_with("abc"));
        assert!(fs::read_to_string(p.join("subdir/hello.txt"))
            .await
            .unwrap()
            .starts_with("def"));

        // Can we extract again without error?
        super::extract_tar_gz_data(test_data.as_slice(), &p).unwrap();

        let _ = fs::remove_dir(p).await;
    }

    #[tokio::test]
    async fn remove_old_build() {
        let p = Path::new("/tmp/shuttle-remove-old-test");

        // Ensure no error occurs with an non-existent directory:

        super::remove_old_build(&p).await.unwrap();

        // Ensure no errors with an empty directory:

        fs::create_dir_all(&p).await.unwrap();

        super::remove_old_build(&p).await.unwrap();

        // Ensure no errror occurs with a marker file pointing to a non-existent
        // file:

        fs::write(p.join(MARKER_FILE_NAME), "i-dont-exist.so")
            .await
            .unwrap();

        super::remove_old_build(&p).await.unwrap();

        assert!(!p.join(MARKER_FILE_NAME).exists());

        // Create a mock marker file and linked library and ensure deletetion
        // occurs correctly:

        fs::write(p.join(MARKER_FILE_NAME), "delete-me.so")
            .await
            .unwrap();
        fs::write(p.join("delete-me.so"), "foobar").await.unwrap();

        assert!(p.join("delete-me.so").exists());

        super::remove_old_build(&p).await.unwrap();

        assert!(!p.join("delete-me").exists());
        assert!(!p.join(MARKER_FILE_NAME).exists());

        let _ = fs::remove_dir(p).await;
    }

    #[tokio::test]
    async fn rename_build() {
        let p = Path::new("/tmp/shuttle-rename-build-test");

        let so_path = p.join("xyz.so");
        let marker_path = p.join(MARKER_FILE_NAME);

        fs::create_dir_all(&p).await.unwrap();
        fs::write(&so_path, "barfoo").await.unwrap();

        super::rename_build(&p, &so_path).await.unwrap();

        // Old '.so' file gone?
        assert!(!so_path.exists());

        // Ensure marker file aligns with the '.so' file's new location:
        let new_so_name = fs::read_to_string(&marker_path).await.unwrap();
        assert_eq!(
            fs::read_to_string(p.join(new_so_name)).await.unwrap(),
            "barfoo"
        );

        let _ = fs::remove_dir(p).await;
    }
}
