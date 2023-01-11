use std::{path::PathBuf, sync::Arc};

use shuttle_proto::runtime::{self, runtime_client::RuntimeClient, SubscribeLogsRequest};
use tokio::sync::Mutex;
use tonic::transport::Channel;

use crate::deployment::deploy_layer;

#[derive(Clone)]
pub struct RuntimeManager {
    legacy: Option<RuntimeClient<Channel>>,
    next: Option<RuntimeClient<Channel>>,
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
            next: None,
            binary_bytes: binary_bytes.to_vec(),
            artifacts_path,
            provisioner_address,
            log_sender,
        }))
    }

    pub async fn get_runtime_client(&mut self, is_next: bool) -> RuntimeClient<Channel> {
        if is_next {
            self.get_next_runtime_client().await
        } else {
            self.get_legacy_runtime_client().await
        }
    }

    async fn get_legacy_runtime_client(&mut self) -> RuntimeClient<Channel> {
        if let Some(ref runtime_client) = self.legacy {
            runtime_client.clone()
        } else {
            let (_runtime, mut runtime_client) = runtime::start(
                &self.binary_bytes,
                false,
                runtime::StorageManagerType::Artifacts(self.artifacts_path.clone()),
                &self.provisioner_address,
                6001,
            )
            .await
            .unwrap();

            let sender = self.log_sender.clone();
            let mut stream = runtime_client
                .subscribe_logs(tonic::Request::new(SubscribeLogsRequest {}))
                .await
                .unwrap()
                .into_inner();

            tokio::spawn(async move {
                while let Some(log) = stream.message().await.unwrap() {
                    sender.send(log.into()).expect("to send log to persistence");
                }
            });

            self.legacy = Some(runtime_client.clone());

            runtime_client
        }
    }

    async fn get_next_runtime_client(&mut self) -> RuntimeClient<Channel> {
        if let Some(ref runtime_client) = self.next {
            runtime_client.clone()
        } else {
            let (_runtime, mut runtime_client) = runtime::start(
                &self.binary_bytes,
                true,
                runtime::StorageManagerType::Artifacts(self.artifacts_path.clone()),
                &self.provisioner_address,
                6002,
            )
            .await
            .unwrap();

            let sender = self.log_sender.clone();
            let mut stream = runtime_client
                .subscribe_logs(tonic::Request::new(SubscribeLogsRequest {}))
                .await
                .unwrap()
                .into_inner();

            tokio::spawn(async move {
                while let Some(log) = stream.message().await.unwrap() {
                    sender.send(log.into()).expect("to send log to persistence");
                }
            });

            self.next = Some(runtime_client.clone());

            runtime_client
        }
    }
}
