use super::{Built, QueueReceiver, RunSender, State};
use crate::error::{Error, Result};
use crate::persistence::Persistence;

use shuttle_service::loader::build_crate;
use tracing::{debug, error, info, instrument};

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

/// Path of the directory that contains extracted service Cargo projects.
const BUILDS_PATH: &str = "/tmp/shuttle-builds";

/// The directory in which compiled '.so' files are stored.
const LIBS_PATH: &str = "/tmp/shuttle-libs";

pub async fn task(mut recv: QueueReceiver, persistence: Persistence, run_send: RunSender) {
    info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        let name = queued.name.clone();

        info!("Queued deployment at the front of the queue: {}", name);

        let persistence_clone = persistence.clone();
        let run_send_cloned = run_send.clone();

        tokio::spawn(async move {
            match queued.handle(persistence_clone).await {
                Ok(built) => promote_to_run(built, run_send_cloned).await,
                Err(e) => error!("Error during building of deployment '{}' - {e}", name),
            }
        });
    }
}

#[instrument(fields(name = built.name.as_str(), state = %State::Built))]
async fn promote_to_run(built: Built, run_send: RunSender) {
    run_send.send(built).await.unwrap();
}

pub struct Queued {
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
}

impl Queued {
    #[instrument(skip(self, persistence), fields(name = self.name.as_str(), state = %State::Building))]
    async fn handle(mut self, persistence: Persistence) -> Result<Built> {
        fs::create_dir_all(BUILDS_PATH).await?;
        fs::create_dir_all(LIBS_PATH).await?;

        info!("Fetching POSTed data");

        let mut vec = Vec::new();
        while let Some(buf) = self.data_stream.next().await {
            let buf = buf?;
            debug!("Received {} bytes", buf.len());
            vec.put(buf);
        }

        info!("Extracting received data");

        let project_path = PathBuf::from(BUILDS_PATH).join(&self.name);

        extract_tar_gz_data(vec.as_slice(), &project_path)?;

        info!("Building deployment");

        let cargo_output_buf = Box::new(std::io::stdout()); // TODO: Redirect over WebSocket.

        let project_path = project_path.canonicalize()?;
        let so_path =
            build_crate(&project_path, cargo_output_buf).map_err(|e| Error::Build(e.into()))?;

        info!("Removing old build (if present)");

        remove_old_lib(&persistence, &self.name, LIBS_PATH).await?;

        info!("Moving built library and storing its location in the database");

        store_lib(&persistence, &self.name, so_path, LIBS_PATH).await?;

        let built = Built { name: self.name };

        Ok(built)
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Queued {{ name: \"{}\", .. }}", self.name)
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

/// Check for a '.so' file specified in the database for the given deployment
/// name and, if one is found, delete it from the libs directory.
async fn remove_old_lib(
    persistence: &Persistence,
    name: &str,
    libs_path: impl AsRef<Path>,
) -> Result<()> {
    if let Some(lib_name) = persistence.get_last_built_lib(name).await? {
        let lib_path = libs_path.as_ref().join(lib_name);

        if lib_path.exists() {
            fs::remove_file(lib_path).await?;
        }
    }

    Ok(())
}

/// Give the '.so' file specified a random name (so that re-deployments are
/// properly re-loaded) and move it to the specified storage directory. Will
/// the chosen '.so' file name in the database.
async fn store_lib(
    persistence: &Persistence,
    name: &str,
    old_so_path: impl AsRef<Path>,
    storage_dir_path: impl AsRef<Path>,
) -> Result<()> {
    let random_so_name =
        rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let new_so_path = storage_dir_path.as_ref().join(&random_so_name);

    fs::rename(old_so_path, new_so_path).await?;

    persistence.set_last_built_lib(name, &random_so_name).await
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use tokio::fs;

    use crate::persistence::Persistence;

    #[tokio::test]
    async fn extract_tar_gz_data() {
        let dir = TempDir::new("extract-tar-gz-data").unwrap();
        let p = dir.path();

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

        fs::remove_dir_all(p).await.unwrap();
    }

    #[tokio::test]
    async fn remove_old_lib() {
        let dir = TempDir::new("remove-old-lib").unwrap();
        let p = dir.path();

        let (db, _) = Persistence::new_in_memory().await;

        // Ensure no errors with a non-existent deployment:

        super::remove_old_lib(&db, "abc", p).await.unwrap();

        // Ensure no errors with a not-yet-built deployment / non-existent '.so'
        // file:

        db.set_last_built_lib("abc", "abc.so").await.unwrap();
        super::remove_old_lib(&db, "abc", p).await.unwrap();

        // Ensure proper removal of a '.so' file:

        fs::write(p.join("abc.so"), "delete me plz").await.unwrap();
        fs::write(p.join("not-me.so"), "don't delete me!!!")
            .await
            .unwrap();

        super::remove_old_lib(&db, "abc", p).await.unwrap();

        assert!(!p.join("abc.so").exists());
        assert!(p.join("not-me.so").exists());
    }

    #[tokio::test]
    async fn store_lib() {
        let libs_dir = TempDir::new("store-lib-libs").unwrap();
        let libs_p = libs_dir.path();

        let build_dir = TempDir::new("store-lib-build").unwrap();
        let build_p = build_dir.path();

        let (db, _) = Persistence::new_in_memory().await;

        let so_path = build_p.join("abc.so");
        fs::write(&so_path, "barfoo").await.unwrap();

        super::store_lib(&db, "abc", &so_path, libs_p)
            .await
            .unwrap();

        // Old '.so' file gone?
        assert!(!so_path.exists());

        // Ensure the '.so' file with its new name has been correctly placed in
        // the libs directory:
        let new_so_path = libs_p.join(db.get_last_built_lib("abc").await.unwrap().unwrap());
        assert_eq!(&fs::read_to_string(new_so_path).await.unwrap(), "barfoo");

        // Can we do the same thing again?

        fs::write(&so_path, "foobar").await.unwrap();

        super::store_lib(&db, "abc", &so_path, libs_p)
            .await
            .unwrap();

        let new_so_path = libs_p.join(db.get_last_built_lib("abc").await.unwrap().unwrap());
        assert_eq!(&fs::read_to_string(new_so_path).await.unwrap(), "foobar");
    }
}
