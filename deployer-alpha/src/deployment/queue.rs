use super::deploy_layer::{Log, LogRecorder, LogType};
use super::gateway_client::BuildQueueClient;
use super::{Built, QueueReceiver, RunSender, State};
use crate::error::{Error, Result, TestError};
use crate::persistence::{DeploymentUpdater, LogLevel, SecretRecorder};
use shuttle_common::storage_manager::{ArtifactsStorageManager, StorageManager};

use cargo_metadata::Message;
use chrono::Utc;
use crossbeam_channel::Sender;
use opentelemetry::global;
use serde_json::json;
use shuttle_common::claims::Claim;
use shuttle_service::builder::{build_workspace, BuiltService};
use tokio::time::{sleep, timeout};
use tracing::{debug, debug_span, error, info, instrument, trace, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs::remove_file;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use tokio::fs;

pub async fn task(
    mut recv: QueueReceiver,
    run_send: RunSender,
    deployment_updater: impl DeploymentUpdater,
    log_recorder: impl LogRecorder,
    secret_recorder: impl SecretRecorder,
    storage_manager: ArtifactsStorageManager,
    queue_client: impl BuildQueueClient,
) {
    info!("Queue task started");

    while let Some(queued) = recv.recv().await {
        let id = queued.id;

        info!("Queued deployment at the front of the queue: {id}");

        let deployment_updater = deployment_updater.clone();
        let run_send_cloned = run_send.clone();
        let log_recorder = log_recorder.clone();
        let secret_recorder = secret_recorder.clone();
        let storage_manager = storage_manager.clone();
        let queue_client = queue_client.clone();

        tokio::spawn(async move {
            let parent_cx = global::get_text_map_propagator(|propagator| {
                propagator.extract(&queued.tracing_context)
            });
            let span = debug_span!("builder");
            span.set_parent(parent_cx);

            async move {
                match timeout(
                    Duration::from_secs(60 * 3), // Timeout after 3 minutes if the build queue hangs or it takes too long for a slot to become available
                    wait_for_queue(queue_client.clone(), id),
                )
                .await
                {
                    Ok(_) => {}
                    Err(err) => return build_failed(&id, err),
                }

                match queued
                    .handle(
                        storage_manager,
                        deployment_updater,
                        log_recorder,
                        secret_recorder,
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
    }
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Crashed))]
fn build_failed(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "service build encountered an error"
    );
}

#[instrument(skip(queue_client), fields(state = %State::Queued))]
async fn wait_for_queue(queue_client: impl BuildQueueClient, id: Uuid) -> Result<()> {
    trace!("getting a build slot");
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

async fn remove_from_queue(queue_client: impl BuildQueueClient, id: Uuid) {
    match queue_client.release_slot(id).await {
        Ok(_) => {}
        Err(error) => warn!(
            error = &error as &dyn std::error::Error,
            "could not release build slot"
        ),
    }
}

#[instrument(skip(run_send), fields(id = %built.id, state = %State::Built))]
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
    pub service_id: Uuid,
    pub data: Vec<u8>,
    pub will_run_tests: bool,
    pub tracing_context: HashMap<String, String>,
    pub claim: Option<Claim>,
}

impl Queued {
    #[instrument(skip(self, storage_manager, deployment_updater, log_recorder, secret_recorder), fields(id = %self.id, state = %State::Building))]
    async fn handle(
        self,
        storage_manager: ArtifactsStorageManager,
        deployment_updater: impl DeploymentUpdater,
        log_recorder: impl LogRecorder,
        secret_recorder: impl SecretRecorder,
    ) -> Result<Built> {
        info!("Extracting received data");

        let project_path = storage_manager.service_build_path(&self.service_name)?;

        info!("Building deployment");

        let (tx, rx): (crossbeam_channel::Sender<Message>, _) = crossbeam_channel::bounded(0);
        let id = self.id;
        tokio::task::spawn_blocking(move || {
            while let Ok(message) = rx.recv() {
                trace!(?message, "received cargo message");
                // TODO: change these to `info!(...)` as [valuable] support increases.
                // Currently it is not possible to turn these serde `message`s into a `valuable`, but once it is the passing down of `log_recorder` should be removed.
                let log = match message {
                    Message::TextLine(line) => Log {
                        id,
                        state: State::Building,
                        level: LogLevel::Info,
                        timestamp: Utc::now(),
                        file: None,
                        line: None,
                        target: String::new(),
                        fields: json!({ "build_line": line }),
                        r#type: LogType::Event,
                    },
                    message => Log {
                        id,
                        state: State::Building,
                        level: LogLevel::Debug,
                        timestamp: Utc::now(),
                        file: None,
                        line: None,
                        target: String::new(),
                        fields: serde_json::to_value(message).unwrap(),
                        r#type: LogType::Event,
                    },
                };
                log_recorder.record(log);
            }
        });

        let project_path = project_path.canonicalize()?;

        // Currently returns the first found shuttle service in a given workspace.
        let runtime = build_deployment(&project_path, tx.clone()).await?;

        // Set the secrets from the service, ignoring any Secrets.toml if it is in the root of the workspace.
        // TODO: refactor this when we support starting multiple services. Do we want to set secrets in the
        // workspace root?
        set_secrets(secrets, &self.service_id, secret_recorder).await?;

        if self.will_run_tests {
            info!(
                build_line = "Running tests before starting up",
                "Running deployment's unit tests"
            );

            run_pre_deploy_tests(&project_path, tx).await?;
        }

        let is_next = runtime.is_wasm;

        deployment_updater
            .set_is_next(&id, is_next)
            .await
            .map_err(|e| Error::Build(Box::new(e)))?;

        let built = Built {
            id: self.id,
            service_name: self.service_name,
            service_id: self.service_id,
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

#[instrument(skip(secrets, service_id, secret_recorder))]
async fn set_secrets(
    secrets: BTreeMap<String, String>,
    service_id: &Uuid,
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

#[instrument(skip(project_path, tx))]
async fn build_deployment(
    project_path: &Path,
    tx: crossbeam_channel::Sender<Message>,
) -> Result<BuiltService> {
    let runtimes = build_workspace(project_path, true, tx, true)
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

    // This needs to be on a separate thread, else deployer will block (reason currently unknown :D)
    tokio::task::spawn_blocking(move || {
        for message in Message::parse_stream(read) {
            match message {
                Ok(message) => {
                    if let Err(error) = tx.send(message) {
                        error!("failed to send cargo message on channel: {error}");
                    }
                }
                Err(error) => {
                    error!("failed to parse cargo message: {error}");
                }
            }
        }
    });

    let mut cmd = Command::new("cargo")
        .arg("test")
        .arg("--release")
        .arg("--jobs=4")
        .arg("--message-format=json")
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::Write, path::Path};

    use shuttle_common::storage_manager::ArtifactsStorageManager;
    use tempfile::Builder;
    use tokio::fs;
    use uuid::Uuid;

    use crate::error::TestError;

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
}
