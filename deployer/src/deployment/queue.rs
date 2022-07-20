use super::deploy_layer::{Log, LogRecorder, LogType};
use super::log::Level;
use super::{Built, QueueReceiver, RunSender, State};
use crate::error::{Error, Result};

use cargo_metadata::Message;
use chrono::Utc;
use serde_json::json;
use shuttle_service::loader::build_crate;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use bytes::{BufMut, Bytes};
use cargo::core::compiler::CompileMode;
use cargo::core::Workspace;
use cargo::ops::{CompileOptions, TestOptions};
use cargo::util::config::Config as CargoConfig;
use flate2::read::GzDecoder;
use futures::{Stream, StreamExt};
use tar::Archive;
use tokio::fs;

/// Path of the directory that contains extracted service Cargo projects.
const BUILDS_PATH: &str = "/tmp/shuttle-builds";

/// The directory in which compiled '.so' files are stored.
pub const LIBS_PATH: &str = "/tmp/shuttle-libs";

pub async fn task(mut recv: QueueReceiver, run_send: RunSender, log_recorder: impl LogRecorder) {
    info!("Queue task started");

    fs::create_dir_all(BUILDS_PATH)
        .await
        .expect("could not create builds directory");
    fs::create_dir_all(LIBS_PATH)
        .await
        .expect("could not create libs directory");

    while let Some(queued) = recv.recv().await {
        let id = queued.id;

        info!("Queued deployment at the front of the queue: {id}");

        let run_send_cloned = run_send.clone();
        let log_recorder = log_recorder.clone();

        tokio::spawn(async move {
            match queued.handle(log_recorder).await {
                Ok(built) => promote_to_run(built, run_send_cloned).await,
                Err(err) => build_failed(&id, err),
            }
        });
    }
}

#[instrument(fields(id = %_id, state = %State::Crashed))]
fn build_failed(_id: &Uuid, err: impl std::error::Error + 'static) {
    error!(
        error = &err as &dyn std::error::Error,
        "service build encountered an error"
    );
}

#[instrument(fields(id = %built.id, state = %State::Built))]
async fn promote_to_run(built: Built, run_send: RunSender) {
    if let Err(err) = run_send.send(built.clone()).await {
        build_failed(&built.id, err);
    }
}

pub struct Queued {
    pub id: Uuid,
    pub name: String,
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
    pub will_run_tests: bool,
}

impl Queued {
    #[instrument(name = "queued_handle", skip(self, log_recorder), fields(id = %self.id, state = %State::Building))]
    async fn handle(self, log_recorder: impl LogRecorder) -> Result<Built> {
        info!("Fetching POSTed data");

        let vec = extract_stream(self.data_stream).await?;

        info!("Extracting received data");

        let project_path = PathBuf::from(BUILDS_PATH).join(&self.name);
        fs::create_dir_all(project_path.clone()).await?;

        extract_tar_gz_data(vec.as_slice(), &project_path)?;

        info!("Building deployment");

        let (tx, mut rx): (UnboundedSender<Message>, _) = mpsc::unbounded_channel();
        let id = self.id;
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                // TODO: change these to `info!(...)` as [valuable] support increases.
                // Currently it is not possible to turn these serde `message`s into a `valuable`, but once it is the passing down of `log_recorder` should be removed.
                match message {
                    Message::TextLine(line) => log_recorder.record(Log {
                        id,
                        state: State::Building,
                        level: Level::Info,
                        timestamp: Utc::now(),
                        file: None,
                        line: None,
                        fields: json!({ "build_line": line }),
                        r#type: LogType::Event,
                    }),
                    message => log_recorder.record(Log {
                        id,
                        state: State::Building,
                        level: Level::Debug,
                        timestamp: Utc::now(),
                        file: None,
                        line: None,
                        fields: serde_json::to_value(message).unwrap(),
                        r#type: LogType::Event,
                    }),
                }
            }
        });

        let project_path = project_path.canonicalize()?;
        let so_path = build_deployment(&project_path, tx).await?;

        if self.will_run_tests {
            info!("Running deployment's unit tests");

            run_pre_deploy_tests(&project_path)?;
        }

        info!("Moving built library");

        store_lib(LIBS_PATH, so_path, &self.id).await?;

        let built = Built {
            id: self.id,
            name: self.name,
        };

        Ok(built)
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Queued {{ id: \"{}\", name: \"{}\", .. }}",
            self.id, self.name
        )
    }
}

#[instrument(skip(data_stream))]
async fn extract_stream(
    mut data_stream: Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>,
) -> Result<Vec<u8>> {
    let mut vec = Vec::new();
    while let Some(buf) = data_stream.next().await {
        let buf = buf?;
        debug!("Received {} bytes", buf.len());
        vec.put(buf);
    }

    Ok(vec)
}

/// Equivalent to the command: `tar -xzf --strip-components 1`
#[instrument(skip(data, dest))]
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

#[instrument(skip(project_path, tx))]
async fn build_deployment(project_path: &Path, tx: UnboundedSender<Message>) -> Result<PathBuf> {
    let so_path = build_crate(&project_path, tx)
        .await
        .map_err(|e| Error::Build(e.into()))?;

    Ok(so_path)
}

#[instrument(skip(project_path))]
fn run_pre_deploy_tests(project_path: impl AsRef<Path>) -> Result<()> {
    let config = CargoConfig::default().map_err(|e| Error::Build(e.into()))?;
    let manifest_path = project_path.as_ref().join("Cargo.toml");

    let ws = Workspace::new(&manifest_path, &config).map_err(|e| Error::Build(e.into()))?;

    let opts = TestOptions {
        compile_opts: CompileOptions::new(&config, CompileMode::Test)
            .map_err(|e| Error::Build(e.into()))?,
        no_run: false,
        no_fail_fast: false,
    };

    let test_failures =
        cargo::ops::run_tests(&ws, &opts, &[]).map_err(|e| Error::Build(e.into()))?;

    match test_failures {
        Some(failures) => Err(failures.into()),
        None => Ok(()),
    }
}

/// Store 'so' file in the libs folder
#[instrument(skip(storage_dir_path, so_path, id))]
async fn store_lib(
    storage_dir_path: impl AsRef<Path>,
    so_path: impl AsRef<Path>,
    id: &Uuid,
) -> Result<()> {
    let new_so_path = storage_dir_path.as_ref().join(id.to_string());

    fs::rename(so_path, new_so_path).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempdir::TempDir;
    use tokio::fs;
    use uuid::Uuid;

    use crate::error::Error;

    #[tokio::test]
    async fn extract_tar_gz_data() {
        let dir = TempDir::new("shuttle-extraction-test").unwrap();
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
    }

    #[tokio::test]
    async fn run_pre_deploy_tests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));

        let failure_project_path = root.join("tests/resources/tests-fail");
        assert!(matches!(
            super::run_pre_deploy_tests(failure_project_path),
            Err(Error::PreDeployTestFailure(_))
        ));

        let pass_project_path = root.join("tests/resources/tests-pass");
        super::run_pre_deploy_tests(pass_project_path).unwrap();
    }

    #[tokio::test]
    async fn store_lib() {
        let libs_dir = TempDir::new("lib-store").unwrap();
        let libs_p = libs_dir.path();

        let build_dir = TempDir::new("build-store").unwrap();
        let build_p = build_dir.path();

        let so_path = build_p.join("xyz.so");
        let id = Uuid::new_v4();

        fs::write(&so_path, "barfoo").await.unwrap();

        super::store_lib(&libs_p, &so_path, &id).await.unwrap();

        // Old '.so' file gone?
        assert!(!so_path.exists());

        assert_eq!(
            fs::read_to_string(libs_p.join(id.to_string()))
                .await
                .unwrap(),
            "barfoo"
        );
    }
}
