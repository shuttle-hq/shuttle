use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use cargo_metadata::Message;
use crossbeam_channel::Sender;
use flate2::read::GzDecoder;
use opentelemetry::global;
use shuttle_common::{
    claims::Claim,
    constants::{EXECUTABLE_DIRNAME, STORAGE_DIRNAME},
    deployment::DEPLOYER_END_MSG_BUILD_ERR,
    log::LogRecorder,
    LogItem,
};
use shuttle_proto::builder::builder_client::BuilderClient;
use shuttle_proto::builder::BuildRequest;
use shuttle_service::builder::{build_workspace, BuiltService};
use tar::Archive;
use tokio::{
    fs,
    task::JoinSet,
    time::{sleep, timeout},
};
use tonic::Request;
use tracing::{debug, debug_span, error, info, instrument, trace, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;
use uuid::Uuid;

use super::gateway_client::BuildQueueClient;
use super::{Built, QueueReceiver, RunSender, State};
use crate::error::{Error, Result, TestError};
use crate::persistence::{DeploymentUpdater, SecretRecorder};

#[allow(clippy::too_many_arguments)]
pub async fn task(
    mut recv: QueueReceiver,
    run_send: RunSender,
    deployment_updater: impl DeploymentUpdater,
    log_recorder: impl LogRecorder,
    secret_recorder: impl SecretRecorder,
    queue_client: impl BuildQueueClient,
    builder_client: Option<
        BuilderClient<
            shuttle_common::claims::ClaimService<
                shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
            >,
        >,
    >,
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
                let secret_recorder = secret_recorder.clone();
                let queue_client = queue_client.clone();
                let builds_path = builds_path.clone();
                let builder_client = builder_client.clone();

                tasks.spawn(async move {
                    let parent_cx = global::get_text_map_propagator(|propagator| {
                        propagator.extract(&queued.tracing_context)
                    });
                    let span = debug_span!("builder");
                    span.set_parent(parent_cx);

                    async move {
                        // Timeout after 3 minutes if the build queue hangs or it takes
                        // too long for a slot to become available
                        match timeout(
                            Duration::from_secs(60 * 3),
                            wait_for_queue(queue_client.clone(), id),
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(err) => return build_failed(&id, err),
                        }

                        if let Some(mut inner) = builder_client {
                            let deployment_id = queued.id.to_string();
                            let archive = queued.data.clone();
                            let claim = queued.claim.clone();
                            tokio::spawn(async move {
                                let mut req = Request::new(BuildRequest {
                                    deployment_id,
                                    archive,
                                });
                                req.extensions_mut().insert(claim);

                                match inner.build(req).await {
                                    Ok(inner) =>  {
                                        let response = inner.into_inner();
                                        info!(id = %queued.id, "shuttle-builder finished building the deployment: image length is {} bytes, is_wasm flag is {} and there are {} secrets", response.image.len(), response.is_wasm, response.secrets.len());
                                    },
                                    Err(err) => error!(id = %queued.id, "shuttle-builder errored while building: {}", err)
                                };
                            });
                        }

                        match queued
                            .handle(
                                deployment_updater,
                                log_recorder,
                                secret_recorder,
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
                    Err(err) => error!(error = %err, "an error happened while joining a builder task"),
                }
            }
            else => break
        }
    }
}

#[instrument(name = "Build failed", skip(_id), fields(deployment_id = %_id, state = %State::Crashed))]
fn build_failed(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "{}", DEPLOYER_END_MSG_BUILD_ERR,
    );
}

#[instrument(name = "Waiting for queue slot", skip(queue_client), fields(deployment_id = %id, state = %State::Queued))]
async fn wait_for_queue(queue_client: impl BuildQueueClient, id: Uuid) -> Result<()> {
    loop {
        let got_slot = queue_client.get_slot(id).await?;

        if got_slot {
            break;
        }

        info!("The build queue is currently full...");

        sleep(Duration::from_secs(1)).await;
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
        skip(self, deployment_updater, log_recorder, secret_recorder, builds_path),
        fields(deployment_id = %self.id, state = %State::Building)
    )]
    async fn handle(
        self,
        deployment_updater: impl DeploymentUpdater,
        log_recorder: impl LogRecorder,
        secret_recorder: impl SecretRecorder,
        builds_path: &Path,
    ) -> Result<Built> {
        let project_path = builds_path.join(&self.service_name);

        info!("Extracting files");
        fs::create_dir_all(&project_path).await?;
        extract_tar_gz_data(self.data.as_slice(), &project_path).await?;

        let (tx, rx): (crossbeam_channel::Sender<Message>, _) = crossbeam_channel::bounded(0);

        tokio::task::spawn_blocking(move || {
            while let Ok(message) = rx.recv() {
                trace!(?message, "received cargo message");
                let log = LogItem::new(
                    self.id,
                    shuttle_common::log::Backend::Deployer, // will change to Builder
                    match message {
                        Message::TextLine(line) => line,
                        message => serde_json::to_string(&message).unwrap(),
                    },
                );
                log_recorder.record(log);
            }
        });

        let project_path = project_path.canonicalize()?;

        info!("Building deployment");
        // Currently returns the first found shuttle service in a given workspace.
        let built_service = build_deployment(&project_path, tx.clone()).await?;

        // Get the Secrets.toml from the shuttle service in the workspace.
        let secrets = get_secrets(built_service.crate_directory()).await?;

        // Set the secrets from the service, ignoring any Secrets.toml if it is in the root of the workspace.
        // TODO: refactor this when we support starting multiple services. Do we want to set secrets in the
        // workspace root?
        set_secrets(secrets, &self.service_id, secret_recorder).await?;

        if self.will_run_tests {
            info!("Running tests before starting up");
            run_pre_deploy_tests(&project_path, tx).await?;
        }

        info!("Moving built executable");
        move_executable(
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
            claim: self.claim,
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
async fn get_secrets(project_path: &Path) -> Result<BTreeMap<String, String>> {
    let secrets_file = project_path.join("Secrets.toml");

    if secrets_file.exists() && secrets_file.is_file() {
        let secrets_str = fs::read_to_string(secrets_file.clone()).await?;

        let secrets: BTreeMap<String, String> = secrets_str.parse::<toml::Value>()?.try_into()?;

        fs::remove_file(secrets_file).await?;

        Ok(secrets)
    } else {
        Ok(Default::default())
    }
}

#[instrument(skip(secrets, service_id, secret_recorder))]
async fn set_secrets(
    secrets: BTreeMap<String, String>,
    service_id: &Ulid,
    secret_recorder: impl SecretRecorder,
) -> Result<()> {
    for (key, value) in secrets.into_iter() {
        debug!(key, "setting secret");

        secret_recorder
            .insert_secret(service_id, &key, &value)
            .await
            .map_err(|e| Error::SecretsSet(Box::new(e)))?;
    }

    Ok(())
}

/// Akin to the command: `tar -xzf --strip-components 1`
#[instrument(skip(data, dest))]
async fn extract_tar_gz_data(data: impl Read, dest: impl AsRef<Path>) -> Result<()> {
    debug!("Unpacking archive into {:?}", dest.as_ref());
    let tar = GzDecoder::new(data);
    let mut archive = Archive::new(tar);
    archive.set_overwrite(true);

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
    tx: crossbeam_channel::Sender<Message>,
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
    tx: Sender<Message>,
) -> std::result::Result<(), TestError> {
    let (read, write) = pipe::pipe();
    let project_path = project_path.to_owned();

    tokio::task::spawn_blocking(move || {
        for line in read.lines() {
            match line {
                Ok(line) => {
                    if let Err(error) = tx.send(Message::TextLine(line)) {
                        error!("failed to send cargo message on channel: {error}");
                    }
                }
                Err(error) => {
                    error!("failed to read cargo output line: {error}");
                }
            }
        }
    });

    let mut cmd = Command::new("cargo")
        .arg("test")
        // We set the tests to build with the release profile since deployments compile
        // with the release profile by default. This means crates don't need to be
        // recompiled in debug mode for the tests, reducing memory usage during deployment.
        // When running unit tests, it can compile in debug mode.
        .arg(if cfg!(not(test)) { "--release" } else { "" })
        .arg("--jobs=4")
        .arg("--color=always")
        .current_dir(project_path)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(TestError::Run)?;

    let stdout = cmd.stdout.take().unwrap();
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines().flatten() {
        if let Err(error) = write.send(format!("{}\n", line.trim_end_matches('\n'))) {
            error!("failed to send to pipe: {error}");
        }
    }

    if cmd.wait().map_err(TestError::Run)?.success() {
        Ok(())
    } else {
        Err(TestError::Failed)
    }
}

/// This will store the path to the executable for each runtime, which will be the users project with
/// an embedded runtime for alpha, and a .wasm file for shuttle-next.
#[instrument(skip(executable_path, to_directory, new_filename))]
async fn move_executable(
    executable_path: &Path,
    to_directory: &Path,
    new_filename: &Uuid,
) -> Result<()> {
    fs::create_dir_all(to_directory).await?;
    fs::rename(executable_path, to_directory.join(new_filename.to_string())).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::Write, path::Path};

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
        let (tx, rx) = crossbeam_channel::unbounded();

        tokio::task::spawn_blocking(move || while rx.recv().is_ok() {});

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

        super::move_executable(executable_path.as_path(), executables_p, &id)
            .await
            .unwrap();

        // Old executable file gone?
        assert!(!executable_path.exists());

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
        let expected = BTreeMap::from([("KEY".to_string(), "value".to_string())]);

        assert_eq!(actual, expected);

        assert!(!secret_p.exists(), "the secrets file should be deleted");
    }
}
