use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use chrono::Utc;
use prost_types::Timestamp;
use shuttle_common::log::Backend;
use shuttle_proto::{
    logger::{self, Batcher, LogItem, LogLine},
    runtime::{self, StopRequest},
};
use shuttle_service::runner;
use tokio::{io::AsyncBufReadExt, io::BufReader, process, sync::Mutex};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

type Runtimes = Arc<std::sync::Mutex<HashMap<Uuid, (process::Child, runtime::Client)>>>;

/// Manager that can start up multiple runtimes. This is needed so that two runtimes can be up when a new deployment is made:
/// One runtime for the new deployment being loaded; another for the currently active deployment
#[derive(Clone)]
pub struct RuntimeManager {
    runtimes: Runtimes,
    logger_client: Batcher<logger::Client>,
}

impl RuntimeManager {
    pub fn new(logger_client: Batcher<logger::Client>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            runtimes: Default::default(),
            logger_client,
        }))
    }

    pub async fn create_runtime_client(
        &mut self,
        id: Uuid,
        project_path: &Path,
        service_name: String,
        runtime_executable: PathBuf,
    ) -> anyhow::Result<runtime::Client> {
        trace!("making new client");

        // the port to run the runtime's gRPC server on
        let port =
            portpicker::pick_unused_port().context("failed to find port for runtime server")?;

        debug!(
            "Starting alpha runtime at: {}",
            runtime_executable
                .clone()
                .into_os_string()
                .into_string()
                .unwrap_or_default()
        );

        let (mut process, runtime_client) = runner::start(
            false,
            port,
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
            runtime_executable,
            project_path,
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

    pub fn kill_process(&mut self, id: Uuid) {
        if let Some((mut process, _)) = self.runtimes.lock().unwrap().remove(&id) {
            match process.start_kill() {
                Ok(_) => info!(deployment_id = %id, "initiated runtime process killing"),
                Err(err) => error!(
                    deployment_id = %id,
                    error = &err as &dyn std::error::Error,
                    "failed to start the killing of the runtime",

                ),
            }
        }
    }

    /// Send a kill / stop signal for a deployment to its running runtime
    pub async fn kill(&mut self, id: &Uuid) -> bool {
        let value = self.runtimes.lock().unwrap().remove(id);

        let Some((mut process, mut runtime_client)) = value else {
            trace!("no client running");
            return true;
        };

        trace!(%id, "sending stop signal for deployment");
        let stop_request = tonic::Request::new(StopRequest {});
        let Ok(response) = runtime_client.stop(stop_request).await else {
            warn!(%id, "stop request failed");
            return false;
        };
        trace!(?response, "stop deployment response");

        let _ = process.start_kill();

        response.into_inner().success
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
