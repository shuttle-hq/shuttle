use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use flate2::read::GzDecoder;
use opentelemetry::global;
use shuttle_common::{
    claims::Claim,
    constants::{EXECUTABLE_DIRNAME, STORAGE_DIRNAME},
    deployment::DEPLOYER_END_MSG_BUILD_ERR,
    log::LogRecorder,
    LogItem,
};
use shuttle_service::builder::{build_workspace, BuiltService};
use tar::Archive;
use tokio::{
    fs,
    io::AsyncBufReadExt,
    task::JoinSet,
    time::{sleep, timeout},
};
use tracing::{debug, debug_span, error, info, instrument, trace, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;
use uuid::Uuid;

use super::gateway_client::BuildQueueClient;
use super::{Built, QueueReceiver, RunSender, State};
use crate::error::{Error, Result, TestError};
use crate::persistence::DeploymentUpdater;

pub async fn task(
    mut recv: QueueReceiver,
    run_send: RunSender,
    deployment_updater: impl DeploymentUpdater,
    log_recorder: impl LogRecorder,
    queue_client: impl BuildQueueClient,
    builds_path: PathBuf,
) {
    info!("Queue task started");

    let mut tasks = JoinSet::new();

    loop {
        tokio::select! {
            Some(queued) = recv.recv() => {
                let id = queued.id;

                info!("Queued deployment at the front of the queue: {id}");
                let deployment_updater = deployment_updater.clone();
                let run_send_cloned = run_send.clone();
                let log_recorder = log_recorder.clone();
                let queue_client = queue_client.clone();
                let builds_path = builds_path.clone();

                tasks.spawn(async move {
                    let parent_cx = global::get_text_map_propagator(|propagator| {
                        propagator.extract(&queued.tracing_context)
                    });
                    let span = debug_span!("builder");
                    span.set_parent(parent_cx);

                    async move {
                        // Timeout after 5 minutes if the build queue hangs or it takes
                        // too long for a slot to become available
                        if let Err(err) = timeout(
                            Duration::from_secs(60 * 5),
                            wait_for_queue(queue_client.clone(), id),
                        )
                        .await
                        {
                            return build_failed_to_get_slot(&id, err);
                        }

                        match queued
                            .handle(
                                deployment_updater,
                                log_recorder,
                                builds_path.as_path(),
                            )
                            .await
                        {
                            Ok(built) => {
                                remove_from_queue(queue_client, id).await;
                                promote_to_run(built, run_send_cloned).await
                            }
                            Err(err) => {
                                remove_from_queue(queue_client, id).await;
                                build_failed(&id, err)
                            }
                        }
                    }
                    .instrument(span)
                    .await
                });
            },
            Some(res) = tasks.join_next() => {
                match res {
                    Ok(_) => (),
                    Err(err) => error!(error = &err as &dyn std::error::Error, "an error happened while joining a builder task"),
                }
            }
            else => break
        }
    }
}

#[instrument(name = "Build failed", skip(_id), fields(deployment_id = %_id, state = %State::Crashed))]
fn build_failed_to_get_slot(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!("Failed to get a build slot. Giving up. Try to deploy again in 10 minutes");
    error!(
        error = &error as &dyn std::error::Error,
        "{DEPLOYER_END_MSG_BUILD_ERR}"
    );
}

#[instrument(name = "Build failed", skip(_id), fields(deployment_id = %_id, state = %State::Crashed))]
fn build_failed(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "{DEPLOYER_END_MSG_BUILD_ERR}"
    );
}

#[instrument(name = "Waiting for queue slot", skip(queue_client), fields(deployment_id = %id, state = %State::Queued))]
async fn wait_for_queue(queue_client: impl BuildQueueClient, id: Uuid) -> Result<()> {
    loop {
        let got_slot = queue_client.get_slot(id).await?;

        if got_slot {
            break;
        }

        info!("The build queue is currently full. Waiting for a slot..");

        sleep(Duration::from_secs(10)).await;
    }

    Ok(())
}

#[instrument(name = "Releasing queue slot", skip(queue_client), fields(deployment_id = %id))]
async fn remove_from_queue(queue_client: impl BuildQueueClient, id: Uuid) {
    match queue_client.release_slot(id).await {
        Ok(_) => {}
        Err(error) => warn!(
            error = &error as &dyn std::error::Error,
            "could not release build slot"
        ),
    }
}

#[instrument(name = "Starting deployment", skip(run_send), fields(deployment_id = %built.id, state = %State::Built))]
async fn promote_to_run(mut built: Built, run_send: RunSender) {
    let cx = Span::current().context();

    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut built.tracing_context);
    });

    if let Err(err) = run_send.send(built.clone()).await {
        build_failed(&built.id, err);
    }
}

pub struct Queued {
    pub id: Uuid,
    pub service_name: String,
    pub service_id: Ulid,
    pub project_id: Ulid,
    pub data: Vec<u8>,
    pub will_run_tests: bool,
    pub tracing_context: HashMap<String, String>,
    pub claim: Claim,
}

impl Queued {
    #[instrument(
        name = "Building project",
        skip(self, deployment_updater, log_recorder, builds_path),
        fields(deployment_id = %self.id, state = %State::Building)
    )]
    async fn handle(
        self,
        deployment_updater: impl DeploymentUpdater,
        log_recorder: impl LogRecorder,
        builds_path: &Path,
    ) -> Result<Built> {
        let project_path = builds_path.join(&self.service_name);

        info!("Extracting files");
        fs::create_dir_all(&project_path).await?;
        extract_tar_gz_data(self.data.as_slice(), &project_path).await?;

        info!("Building deployment");
        // Listen to build logs
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);
        tokio::task::spawn(async move {
            while let Some(line) = rx.recv().await {
                let log = LogItem::new(
                    self.id,
                    shuttle_common::log::Backend::Deployer, // will change to Builder
                    line,
                );
                log_recorder.record(log);
            }
        });
        let project_path = project_path.canonicalize()?;
        // Currently returns the first found shuttle service in a given workspace.
        let built_service = build_deployment(&project_path, tx.clone()).await?;

        // Get the Secrets.toml from the shuttle service in the workspace.
        let secrets = get_secrets(built_service.crate_directory()).await?;

        if self.will_run_tests {
            info!("Running tests before starting up");
            run_pre_deploy_tests(&project_path, tx).await?;
        }

        info!("Moving built executable");
        copy_executable(
            built_service.executable_path.as_path(),
            built_service
                .workspace_path
                .join(EXECUTABLE_DIRNAME)
                .as_path(),
            &self.id,
        )
        .await?;

        let is_next = built_service.is_wasm;

        deployment_updater
            .set_is_next(&self.id, is_next)
            .await
            .map_err(|e| Error::Build(Box::new(e)))?;

        let built = Built {
            id: self.id,
            service_name: self.service_name,
            service_id: self.service_id,
            project_id: self.project_id,
            tracing_context: Default::default(),
            is_next,
            claim: Some(self.claim),
            secrets,
        };

        Ok(built)
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Queued")
            .field("id", &self.id)
            .field("service_name", &self.service_name)
            .field("service_id", &self.service_id)
            .field("will_run_tests", &self.will_run_tests)
            .finish_non_exhaustive()
    }
}

#[instrument(skip(project_path))]
async fn get_secrets(project_path: &Path) -> Result<HashMap<String, String>> {
    let secrets_file = project_path.join("Secrets.toml");

    if secrets_file.exists() && secrets_file.is_file() {
        let secrets_str = fs::read_to_string(secrets_file.clone()).await?;

        let secrets = secrets_str.parse::<toml::Value>()?.try_into()?;

        fs::remove_file(secrets_file).await?;

        Ok(secrets)
    } else {
        Ok(Default::default())
    }
}

/// Akin to the command: `tar -xzf --strip-components 1`
#[instrument(skip(data, dest))]
async fn extract_tar_gz_data(data: impl Read, dest: impl AsRef<Path>) -> Result<()> {
    // Clear directory first
    trace!("Clearing old files");
    let mut entries = fs::read_dir(&dest).await?;
    while let Some(entry) = entries.next_entry().await? {
        // Ignore files that should be persisted and build cache directory
        if [EXECUTABLE_DIRNAME, STORAGE_DIRNAME, "target", "Cargo.lock"]
            .contains(&entry.file_name().to_string_lossy().as_ref())
        {
            trace!("Skipping {:?} while clearing old files", entry);
            continue;
        }

        if entry.metadata().await?.is_dir() {
            fs::remove_dir_all(entry.path()).await?;
        } else {
            fs::remove_file(entry.path()).await?;
        }
    }

    debug!("Unpacking archive into {:?}", dest.as_ref());
    let tar = GzDecoder::new(data);
    let mut archive = Archive::new(tar);
    archive.set_overwrite(true);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?;
        let path: PathBuf = name.components().skip(1).collect();
        // don't allow archive to overwrite shuttle internals
        if [EXECUTABLE_DIRNAME, STORAGE_DIRNAME, "target"]
            .iter()
            .any(|n| path.starts_with(n))
        {
            info!("Skipping {:?} while unpacking", path);
            continue;
        }
        let dst: PathBuf = dest.as_ref().join(path);
        std::fs::create_dir_all(dst.parent().unwrap())?;
        trace!("Unpacking {:?} to {:?}", name, dst);
        entry.unpack(dst)?;
    }

    Ok(())
}

#[instrument(skip(project_path, tx))]
async fn build_deployment(
    project_path: &Path,
    tx: tokio::sync::mpsc::Sender<String>,
) -> Result<BuiltService> {
    // Build in release mode, except for when testing, such as in CI
    let runtimes = build_workspace(project_path, cfg!(not(test)), tx, true)
        .await
        .map_err(|e| Error::Build(e.into()))?;

    Ok(runtimes[0].clone())
}

#[instrument(skip(project_path, tx))]
async fn run_pre_deploy_tests(
    project_path: &Path,
    tx: tokio::sync::mpsc::Sender<String>,
) -> std::result::Result<(), TestError> {
    let project_path = project_path.to_owned();

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("test")
        // We set the tests to build with the release profile since deployments compile
        // with the release profile by default. This means crates don't need to be
        // recompiled in debug mode for the tests, reducing memory usage during deployment.
        // When running unit tests, it can compile in debug mode.
        .arg(if cfg!(not(test)) { "--release" } else { "" })
        .arg("--jobs=4")
        .arg("--color=always")
        .current_dir(project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Spawn the command and make two readers, that read lines from stdout and stderr and send
    // them to the same receiver. This is only needed when the output of both streams are wanted.
    let mut handle = cmd.spawn().map_err(TestError::Run)?;
    let tx2 = tx.clone();
    let reader = tokio::io::BufReader::new(handle.stdout.take().unwrap());
    tokio::spawn(async move {
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            let _ = tx.send(line).await.map_err(|err| {
                error!(
                    error = &err as &dyn std::error::Error,
                    "failed to send line"
                )
            });
        }
    });
    let reader = tokio::io::BufReader::new(handle.stderr.take().unwrap());
    tokio::spawn(async move {
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            let _ = tx2.send(line).await.map_err(|err| {
                error!(
                    error = &err as &dyn std::error::Error,
                    "failed to send line"
                )
            });
        }
    });
    let status = handle.wait().await.map_err(TestError::Run)?;

    if status.success() {
        Ok(())
    } else {
        Err(TestError::Failed)
    }
}

/// This will store the path to the executable for each runtime, which will be the users project with
/// an embedded runtime for alpha, and a .wasm file for shuttle-next.
#[instrument(skip(executable_path, to_directory, new_filename))]
async fn copy_executable(
    executable_path: &Path,
    to_directory: &Path,
    new_filename: &Uuid,
) -> Result<()> {
    fs::create_dir_all(to_directory).await?;
    fs::copy(executable_path, to_directory.join(new_filename.to_string())).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs::File, io::Write, path::Path};

    use tempfile::Builder;
    use tokio::fs;
    use uuid::Uuid;

    use crate::error::TestError;

    #[tokio::test]
    async fn extract_tar_gz_data() {
        let dir = Builder::new()
            .prefix("shuttle-extraction-test")
            .tempdir()
            .unwrap();
        let p = dir.path();

        // Files whose content should be replaced with the archive
        fs::write(p.join("world.txt"), b"original text")
            .await
            .unwrap();

        // Extra files that should be deleted
        fs::write(
            p.join("extra.txt"),
            b"extra file at top level that should be deleted",
        )
        .await
        .unwrap();
        fs::create_dir_all(p.join("subdir")).await.unwrap();
        fs::write(
            p.join("subdir/extra.txt"),
            b"extra file in subdir that should be deleted",
        )
        .await
        .unwrap();

        // Build cache in `/target` should not be cleared/deleted
        fs::create_dir_all(p.join("target")).await.unwrap();
        fs::write(p.join("target/asset.txt"), b"some file in the build cache")
            .await
            .unwrap();

        // Cargo.lock file shouldn't be deleted
        fs::write(p.join("Cargo.lock"), "lock file contents shouldn't matter")
            .await
            .unwrap();

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

        super::extract_tar_gz_data(test_data.as_slice(), &p)
            .await
            .unwrap();
        assert!(fs::read_to_string(p.join("world.txt"))
            .await
            .unwrap()
            .starts_with("abc"));
        assert!(fs::read_to_string(p.join("subdir/hello.txt"))
            .await
            .unwrap()
            .starts_with("def"));

        assert_eq!(
            fs::metadata(p.join("extra.txt")).await.unwrap_err().kind(),
            std::io::ErrorKind::NotFound,
            "extra file should be deleted"
        );
        assert_eq!(
            fs::metadata(p.join("subdir/extra.txt"))
                .await
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound,
            "extra file in subdir should be deleted"
        );

        assert_eq!(
            fs::read_to_string(p.join("target/asset.txt"))
                .await
                .unwrap(),
            "some file in the build cache",
            "build cache file should not be touched"
        );

        assert_eq!(
            fs::read_to_string(p.join("Cargo.lock")).await.unwrap(),
            "lock file contents shouldn't matter",
            "Cargo lock file should not be touched"
        );

        // Can we extract again without error?
        super::extract_tar_gz_data(test_data.as_slice(), &p)
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_pre_deploy_tests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(256);

        let failure_project_path = root.join("tests/resources/tests-fail");
        assert!(matches!(
            super::run_pre_deploy_tests(&failure_project_path, tx.clone()).await,
            Err(TestError::Failed)
        ));

        let pass_project_path = root.join("tests/resources/tests-pass");
        super::run_pre_deploy_tests(&pass_project_path, tx)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn store_executable() {
        let executables_dir = Builder::new().prefix("executable-store").tempdir().unwrap();
        let executables_p = executables_dir.path();
        let executable_path = executables_p.join("xyz");
        let id = Uuid::new_v4();

        fs::write(&executable_path, "barfoo").await.unwrap();

        super::copy_executable(executable_path.as_path(), executables_p, &id)
            .await
            .unwrap();

        assert_eq!(
            fs::read_to_string(executables_p.join(id.to_string()))
                .await
                .unwrap(),
            "barfoo"
        );
    }

    #[tokio::test]
    async fn get_secrets() {
        let temp = Builder::new().prefix("secrets").tempdir().unwrap();
        let temp_p = temp.path();

        let secret_p = temp_p.join("Secrets.toml");
        let mut secret_file = File::create(secret_p.clone()).unwrap();
        secret_file.write_all(b"KEY = 'value'").unwrap();

        let actual = super::get_secrets(temp_p).await.unwrap();
        let expected = HashMap::from([("KEY".to_string(), "value".to_string())]);

        assert_eq!(actual, expected);

        assert!(!secret_p.exists(), "the secrets file should be deleted");
    }
}
