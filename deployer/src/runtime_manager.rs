use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Context;
use chrono::Utc;
use prost_types::Timestamp;
use shuttle_common::{
    claims::{ClaimService, InjectPropagation},
    log::Backend,
};
use shuttle_proto::{
    logger::{logger_client::LoggerClient, Batcher, LogItem, LogLine},
    runtime::{self, runtime_client::RuntimeClient, StopRequest},
};
use tokio::{io::AsyncBufReadExt, io::BufReader, process, sync::Mutex};
use tonic::transport::Channel;
use tracing::{debug, info, trace};
use uuid::Uuid;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

type Runtimes = Arc<
    std::sync::Mutex<
        HashMap<
            Uuid,
            (
                process::Child,
                RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
            ),
        >,
    >,
>;

/// Manager that can start up mutliple runtimes. This is needed so that two runtimes can be up when a new deployment is made:
/// One runtime for the new deployment being loaded; another for the currently active deployment
#[derive(Clone)]
pub struct RuntimeManager {
    runtimes: Runtimes,
    artifacts_path: PathBuf,
    provisioner_address: String,
    logger_client: Batcher<
        LoggerClient<
            shuttle_common::claims::ClaimService<
                shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
            >,
        >,
    >,
    auth_uri: Option<String>,
}

impl RuntimeManager {
    pub fn new(
        artifacts_path: PathBuf,
        provisioner_address: String,
        logger_client: Batcher<
            LoggerClient<
                shuttle_common::claims::ClaimService<
                    shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
                >,
            >,
        >,
        auth_uri: Option<String>,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            runtimes: Default::default(),
            artifacts_path,
            provisioner_address,
            logger_client,
            auth_uri,
        }))
    }

    pub async fn get_runtime_client(
        &mut self,
        id: Uuid,
        service_name: String,
        alpha_runtime_path: Option<PathBuf>,
    ) -> anyhow::Result<RuntimeClient<ClaimService<InjectPropagation<Channel>>>> {
        trace!("making new client");

        let port = portpicker::pick_unused_port().context("failed to find available port")?;
        let is_next = alpha_runtime_path.is_none();

        let get_runtime_executable = || {
            if let Some(alpha_runtime) = alpha_runtime_path {
                debug!(
                    "Starting alpha runtime at: {}",
                    alpha_runtime
                        .clone()
                        .into_os_string()
                        .into_string()
                        .unwrap_or_default()
                );
                alpha_runtime
            } else {
                if cfg!(debug_assertions) {
                    debug!("Installing shuttle-next runtime in debug mode from local source");
                    // If we're running deployer natively, install shuttle-runtime using the
                    // version of runtime from the calling repo.
                    let path = std::fs::canonicalize(format!("{MANIFEST_DIR}/../runtime"));

                    // The path will not be valid if we are in a deployer container, in which
                    // case we don't try to install and use the one installed in deploy.sh.
                    if let Ok(path) = path {
                        std::process::Command::new("cargo")
                            .arg("install")
                            .arg("shuttle-runtime")
                            .arg("--path")
                            .arg(path)
                            .arg("--bin")
                            .arg("shuttle-next")
                            .arg("--features")
                            .arg("next")
                            .output()
                            .expect("failed to install the local version of shuttle-runtime");
                    }
                }

                debug!("Returning path to shuttle-next runtime",);
                // If we're in a deployer built with the containerfile, the runtime will have
                // been installed in deploy.sh.
                home::cargo_home()
                    .expect("failed to find path to cargo home")
                    .join("bin/shuttle-next")
            }
        };

        let (mut process, runtime_client) = runtime::start(
            is_next,
            runtime::StorageManagerType::Artifacts(self.artifacts_path.clone()),
            &self.provisioner_address,
            self.auth_uri.as_ref(),
            port,
            get_runtime_executable,
        )
        .await
        .context("failed to start shuttle runtime")?;

        let stdout = process
            .stdout
            .take()
            .context("child process did not have a handle to stdout")?;

        self.runtimes
            .lock()
            .unwrap()
            .insert(id, (process, runtime_client.clone()));

        let mut reader = BufReader::new(stdout).lines();
        let logger_client = self.logger_client.clone();
        tokio::spawn(async move {
            while let Some(line) = reader.next_line().await.unwrap() {
                let utc = Utc::now();
                let log = LogItem {
                    deployment_id: id.to_string(),
                    log_line: Some(LogLine {
                        service_name: Backend::Runtime(service_name.clone()).to_string(),
                        tx_timestamp: Some(Timestamp {
                            seconds: utc.timestamp(),
                            nanos: utc.timestamp_subsec_nanos().try_into().unwrap_or_default(),
                        }),
                        data: line.as_bytes().to_vec(),
                    }),
                };
                logger_client.send(log);
            }
        });

        Ok(runtime_client)
    }

    /// Send a kill / stop signal for a deployment to its running runtime
    pub async fn kill(&mut self, id: &Uuid) -> bool {
        let value = self.runtimes.lock().unwrap().remove(id);

        if let Some((mut process, mut runtime_client)) = value {
            trace!(%id, "sending stop signal for deployment");

            let stop_request = tonic::Request::new(StopRequest {});
            let response = runtime_client.stop(stop_request).await.unwrap();

            trace!(?response, "stop deployment response");

            let result = response.into_inner().success;
            let _ = process.start_kill();

            result
        } else {
            trace!("no client running");
            true
        }
    }
}

impl Drop for RuntimeManager {
    fn drop(&mut self) {
        info!("runtime manager shutting down");

        for (process, _runtime_client) in self.runtimes.lock().unwrap().values_mut() {
            let _ = process.start_kill();
        }
    }
}
