use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use shuttle_proto::runtime::{self, runtime_client::RuntimeClient, SubscribeLogsRequest};
use tokio::{process, sync::Mutex};
use tonic::transport::Channel;

use crate::deployment::deploy_layer;

#[derive(Clone)]
pub struct RuntimeManager {
    legacy: Option<RuntimeClient<Channel>>,
    legacy_process: Option<Arc<std::sync::Mutex<process::Child>>>,
    next: Option<RuntimeClient<Channel>>,
    next_process: Option<Arc<std::sync::Mutex<process::Child>>>,
    binary_bytes: Vec<u8>,
    artifacts_path: PathBuf,
    provisioner_address: String,
    log_sender: crossbeam_channel::Sender<deploy_layer::Log>,
}

impl RuntimeManager {
    pub fn new(
        binary_bytes: &[u8],
        artifacts_path: PathBuf,
        provisioner_address: String,
        log_sender: crossbeam_channel::Sender<deploy_layer::Log>,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            legacy: None,
            legacy_process: None,
            next: None,
            next_process: None,
            binary_bytes: binary_bytes.to_vec(),
            artifacts_path,
            provisioner_address,
            log_sender,
        }))
    }

    pub async fn get_runtime_client(
        &mut self,
        is_next: bool,
    ) -> anyhow::Result<RuntimeClient<Channel>> {
        if is_next {
            Self::get_runtime_client_helper(
                &mut self.next,
                &mut self.next_process,
                6002,
                &self.binary_bytes,
                self.artifacts_path.clone(),
                &self.provisioner_address,
                self.log_sender.clone(),
            )
            .await
        } else {
            Self::get_runtime_client_helper(
                &mut self.legacy,
                &mut self.legacy_process,
                6001,
                &self.binary_bytes,
                self.artifacts_path.clone(),
                &self.provisioner_address,
                self.log_sender.clone(),
            )
            .await
        }
    }

    async fn get_runtime_client_helper(
        runtime_option: &mut Option<RuntimeClient<Channel>>,
        process_option: &mut Option<Arc<std::sync::Mutex<process::Child>>>,
        port: u16,
        binary_bytes: &[u8],
        artifacts_path: PathBuf,
        provisioner_address: &str,
        log_sender: crossbeam_channel::Sender<deploy_layer::Log>,
    ) -> anyhow::Result<RuntimeClient<Channel>> {
        if let Some(ref runtime_client) = runtime_option {
            Ok(runtime_client.clone())
        } else {
            let (process, mut runtime_client) = runtime::start(
                binary_bytes,
                true,
                runtime::StorageManagerType::Artifacts(artifacts_path),
                provisioner_address,
                port,
            )
            .await
            .context("failed to start shuttle runtime")?;

            let sender = log_sender;
            let mut stream = runtime_client
                .subscribe_logs(tonic::Request::new(SubscribeLogsRequest {}))
                .await
                .context("subscribing to runtime logs stream")?
                .into_inner();

            tokio::spawn(async move {
                while let Some(log) = stream.message().await.unwrap() {
                    sender.send(log.into()).expect("to send log to persistence");
                }
            });

            *runtime_option = Some(runtime_client.clone());
            *process_option = Some(Arc::new(std::sync::Mutex::new(process)));

            Ok(runtime_client)
        }
    }
}

impl Drop for RuntimeManager {
    fn drop(&mut self) {
        if let Some(ref process) = self.legacy_process {
            let _ = process.lock().unwrap().start_kill();
        }

        if let Some(ref process) = self.next_process {
            let _ = process.lock().unwrap().start_kill();
        }
    }
}
