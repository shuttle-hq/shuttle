use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use async_trait::async_trait;
use opentelemetry::global;
use portpicker::pick_unused_port;
use shuttle_common::{
    claims::{Claim, ClaimService, InjectPropagation},
    resource,
    storage_manager::ArtifactsStorageManager,
};

use shuttle_proto::runtime::{
    runtime_client::RuntimeClient, LoadRequest, StartRequest, StopReason, SubscribeStopRequest,
    SubscribeStopResponse,
};
use tokio::{
    sync::Mutex,
    task::{JoinHandle, JoinSet},
};
use tonic::{transport::Channel, Code};
use tracing::{debug, debug_span, error, info, instrument, trace, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use super::{RunReceiver, State};
use crate::{
    error::{Error, Result},
    persistence::{DeploymentUpdater, Resource, ResourceManager, SecretGetter},
    RuntimeManager,
};

/// Run a task which takes runnable deploys from a channel and starts them up on our runtime
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task(
    mut recv: RunReceiver,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    deployment_updater: impl DeploymentUpdater,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    secret_getter: impl SecretGetter,
    resource_manager: impl ResourceManager,
    storage_manager: ArtifactsStorageManager,
) {
    info!("Run task started");

    let mut set = JoinSet::new();

    while let Some(built) = recv.recv().await {
        let id = built.id;

        info!("Built deployment at the front of run queue: {id}");

        let deployment_updater = deployment_updater.clone();
        let secret_getter = secret_getter.clone();
        let resource_manager = resource_manager.clone();
        let storage_manager = storage_manager.clone();

        let old_deployments_killer = kill_old_deployments(
            built.service_id,
            id,
            active_deployment_getter.clone(),
            runtime_manager.clone(),
        );
        let cleanup = move |response: Option<SubscribeStopResponse>| {
            debug!(response = ?response,  "stop client response: ");

            if let Some(response) = response {
                match StopReason::from_i32(response.reason).unwrap_or_default() {
                    StopReason::Request => stopped_cleanup(&id),
                    StopReason::End => completed_cleanup(&id),
                    StopReason::Crash => crashed_cleanup(
                        &id,
                        Error::Run(anyhow::Error::msg(response.message).into()),
                    ),
                }
            } else {
                crashed_cleanup(
                    &id,
                    Error::Runtime(anyhow::anyhow!(
                        "stop subscribe channel stopped unexpectedly"
                    )),
                )
            }
        };
        let runtime_manager = runtime_manager.clone();

        set.spawn(async move {
            let parent_cx = global::get_text_map_propagator(|propagator| {
                propagator.extract(&built.tracing_context)
            });
            let span = debug_span!("runner");
            span.set_parent(parent_cx);

            async move {
                match built
                    .handle(
                        storage_manager,
                        secret_getter,
                        resource_manager,
                        runtime_manager,
                        deployment_updater,
                        old_deployments_killer,
                        cleanup,
                    )
                    .await
                {
                    Ok(handle) => handle
                        .await
                        .expect("the call to run in built.handle to be done"),
                    Err(err) => start_crashed_cleanup(&id, err),
                };

                info!("deployment done");
            }
            .instrument(span)
            .await
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(_) => (),
            Err(err) => {
                error!(error = %err, "an error happened when joining a deployment run task")
            }
        }
    }
}

#[instrument(skip(active_deployment_getter, runtime_manager))]
async fn kill_old_deployments(
    service_id: Uuid,
    deployment_id: Uuid,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
) -> Result<()> {
    let mut guard = runtime_manager.lock().await;

    for old_id in active_deployment_getter
        .clone()
        .get_active_deployments(&service_id)
        .await
        .map_err(|e| Error::OldCleanup(Box::new(e)))?
        .into_iter()
        .filter(|old_id| old_id != &deployment_id)
    {
        trace!(%old_id, "stopping old deployment");

        if !guard.kill(&old_id).await {
            warn!(id = %old_id, "failed to kill old deployment");
        }
    }

    Ok(())
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Completed))]
fn completed_cleanup(_id: &Uuid) {
    info!("service finished all on its own");
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Stopped))]
fn stopped_cleanup(_id: &Uuid) {
    info!("service was stopped by the user");
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Crashed))]
fn crashed_cleanup(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "service encountered an error"
    );
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Crashed))]
fn start_crashed_cleanup(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "service startup encountered an error"
    );
}

#[async_trait]
pub trait ActiveDeploymentsGetter: Clone + Send + Sync + 'static {
    type Err: std::error::Error + Send;

    async fn get_active_deployments(
        &self,
        service_id: &Uuid,
    ) -> std::result::Result<Vec<Uuid>, Self::Err>;
}

#[derive(Clone, Debug)]
pub struct Built {
    pub id: Uuid,
    pub service_name: String,
    pub service_id: Uuid,
    pub tracing_context: HashMap<String, String>,
    pub is_next: bool,
    pub claim: Option<Claim>,
}

impl Built {
    #[instrument(skip(self, storage_manager, secret_getter, resource_manager, runtime_manager, deployment_updater, kill_old_deployments, cleanup), fields(id = %self.id, state = %State::Loading))]
    #[allow(clippy::too_many_arguments)]
    async fn handle(
        self,
        storage_manager: ArtifactsStorageManager,
        secret_getter: impl SecretGetter,
        resource_manager: impl ResourceManager,
        runtime_manager: Arc<Mutex<RuntimeManager>>,
        deployment_updater: impl DeploymentUpdater,
        kill_old_deployments: impl futures::Future<Output = Result<()>>,
        cleanup: impl FnOnce(Option<SubscribeStopResponse>) + Send + 'static,
    ) -> Result<JoinHandle<()>> {
        // For alpha this is the path to the users project with an embedded runtime.
        // For shuttle-next this is the path to the compiled .wasm file, which will be
        // used in the load request.
        let executable_path = storage_manager.deployment_executable_path(&self.id)?;

        let port = match pick_unused_port() {
            Some(port) => port,
            None => {
                return Err(Error::PrepareRun(
                    "could not find a free port to deploy service on".to_string(),
                ))
            }
        };

        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

        let alpha_runtime_path = if self.is_next {
            // The runtime client for next is the installed shuttle-next bin
            None
        } else {
            Some(executable_path.clone())
        };

        let runtime_client = runtime_manager
            .lock()
            .await
            .get_runtime_client(self.id, alpha_runtime_path.clone())
            .await
            .map_err(Error::Runtime)?;

        kill_old_deployments.await?;

        // Execute loaded service
        load(
            self.service_name.clone(),
            self.service_id,
            executable_path.clone(),
            secret_getter,
            resource_manager,
            runtime_client.clone(),
            self.claim,
        )
        .await?;

        let handler = tokio::spawn(run(
            self.id,
            self.service_name,
            runtime_client,
            address,
            deployment_updater,
            cleanup,
        ));

        Ok(handler)
    }
}

async fn load(
    service_name: String,
    service_id: Uuid,
    executable_path: PathBuf,
    secret_getter: impl SecretGetter,
    resource_manager: impl ResourceManager,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    claim: Option<Claim>,
) -> Result<()> {
    info!(
        "loading project from: {}",
        executable_path
            .clone()
            .into_os_string()
            .into_string()
            .unwrap_or_default()
    );

    // Get resources from cache when a claim is not set (ie an idl project is started)
    let resources = if claim.is_none() {
        resource_manager
            .get_resources(&service_id)
            .await
            .unwrap()
            .into_iter()
            .map(resource::Response::from)
            .map(resource::Response::into_bytes)
            .collect()
    } else {
        Default::default()
    };

    let secrets = secret_getter
        .get_secrets(&service_id)
        .await
        .map_err(|e| Error::SecretsGet(Box::new(e)))?
        .into_iter()
        .map(|secret| (secret.key, secret.value));
    let secrets = HashMap::from_iter(secrets);

    let mut load_request = tonic::Request::new(LoadRequest {
        path: executable_path
            .into_os_string()
            .into_string()
            .unwrap_or_default(),
        service_name: service_name.clone(),
        resources,
        secrets,
    });

    if let Some(claim) = claim {
        load_request.extensions_mut().insert(claim);
    }

    debug!("loading service");
    let response = runtime_client.load(load_request).await;

    match response {
        Ok(response) => {
            let response = response.into_inner();

            // Make sure to not log the entire response, the resources field is likely to contain
            // secrets.
            info!(success = %response.success, "loading response");

            for resource in response.resources {
                let resource: resource::Response = serde_json::from_slice(&resource).unwrap();
                let resource = Resource {
                    service_id,
                    r#type: resource.r#type.into(),
                    config: resource.config,
                    data: resource.data,
                };
                resource_manager
                    .insert_resource(&resource)
                    .await
                    .expect("to add resource to persistence");
            }

            if response.success {
                Ok(())
            } else {
                error!(error = %response.message, "failed to load service");
                Err(Error::Load(response.message))
            }
        }
        Err(error) => {
            error!(%error, "failed to load service");
            Err(Error::Load(error.to_string()))
        }
    }
}

#[instrument(skip(runtime_client, deployment_updater, cleanup), fields(state = %State::Running))]
async fn run(
    id: Uuid,
    service_name: String,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    address: SocketAddr,
    deployment_updater: impl DeploymentUpdater,
    cleanup: impl FnOnce(Option<SubscribeStopResponse>) + Send + 'static,
) {
    deployment_updater
        .set_address(&id, &address)
        .await
        .expect("to set deployment address");

    let start_request = tonic::Request::new(StartRequest {
        ip: address.to_string(),
    });

    // Subscribe to stop before starting to catch immediate errors
    let mut stream = runtime_client
        .subscribe_stop(tonic::Request::new(SubscribeStopRequest {}))
        .await
        .unwrap()
        .into_inner();

    info!("starting service");
    let response = runtime_client.start(start_request).await;

    match response {
        Ok(response) => {
            info!(response = ?response.into_inner(),  "start client response: ");

            // Wait for stop reason
            let reason = stream.message().await.expect("message from tonic stream");

            cleanup(reason);
        }
        Err(ref status) if status.code() == Code::InvalidArgument => {
            cleanup(Some(SubscribeStopResponse {
                reason: StopReason::Crash as i32,
                message: status.to_string(),
            }));
        }
        Err(ref status) => {
            start_crashed_cleanup(
                &id,
                Error::Start("runtime failed to start deployment".to_string()),
            );

            error!(%status, "failed to start service");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv4Addr, SocketAddr},
        path::PathBuf,
        process::Command,
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use portpicker::pick_unused_port;
    use shuttle_common::storage_manager::ArtifactsStorageManager;
    use shuttle_proto::{
        provisioner::{
            provisioner_server::{Provisioner, ProvisionerServer},
            DatabaseDeletionResponse, DatabaseRequest, DatabaseResponse,
        },
        runtime::{StopReason, SubscribeStopResponse},
    };
    use tempfile::Builder;
    use tokio::{
        sync::{oneshot, Mutex},
        time::sleep,
    };
    use tonic::transport::Server;
    use uuid::Uuid;

    use crate::{
        persistence::{DeploymentUpdater, Resource, ResourceManager, Secret, SecretGetter},
        RuntimeManager,
    };

    use super::Built;

    const RESOURCES_PATH: &str = "tests/resources";

    fn get_storage_manager() -> ArtifactsStorageManager {
        let tmp_dir = Builder::new().prefix("shuttle_run_test").tempdir().unwrap();
        let path = tmp_dir.into_path();

        ArtifactsStorageManager::new(path)
    }

    async fn kill_old_deployments() -> crate::error::Result<()> {
        Ok(())
    }

    struct ProvisionerMock;

    #[async_trait]
    impl Provisioner for ProvisionerMock {
        async fn provision_database(
            &self,
            _request: tonic::Request<DatabaseRequest>,
        ) -> Result<tonic::Response<DatabaseResponse>, tonic::Status> {
            panic!("no run tests should request a db");
        }

        async fn delete_database(
            &self,
            _request: tonic::Request<DatabaseRequest>,
        ) -> Result<tonic::Response<DatabaseDeletionResponse>, tonic::Status> {
            panic!("no run tests should delete a db");
        }
    }

    fn get_runtime_manager() -> Arc<Mutex<RuntimeManager>> {
        let provisioner_addr =
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
        let mock = ProvisionerMock;

        tokio::spawn(async move {
            Server::builder()
                .add_service(ProvisionerServer::new(mock))
                .serve(provisioner_addr)
                .await
                .unwrap();
        });

        let tmp_dir = Builder::new().prefix("shuttle_run_test").tempdir().unwrap();
        let path = tmp_dir.into_path();
        let (tx, rx) = crossbeam_channel::unbounded();

        tokio::runtime::Handle::current().spawn_blocking(move || {
            while let Ok(log) = rx.recv() {
                println!("test log: {log:?}");
            }
        });

        RuntimeManager::new(path, format!("http://{}", provisioner_addr), None, tx)
    }

    #[derive(Clone)]
    struct StubSecretGetter;

    #[async_trait]
    impl SecretGetter for StubSecretGetter {
        type Err = std::io::Error;

        async fn get_secrets(&self, _service_id: &Uuid) -> Result<Vec<Secret>, Self::Err> {
            Ok(Default::default())
        }
    }

    #[derive(Clone)]
    struct StubResourceManager;

    #[async_trait]
    impl ResourceManager for StubResourceManager {
        type Err = std::io::Error;

        async fn insert_resource(&self, _resource: &Resource) -> Result<(), Self::Err> {
            Ok(())
        }
        async fn get_resources(&self, _service_id: &Uuid) -> Result<Vec<Resource>, Self::Err> {
            Ok(Vec::new())
        }
    }

    #[derive(Clone)]
    struct StubDeploymentUpdater;

    #[async_trait]
    impl DeploymentUpdater for StubDeploymentUpdater {
        type Err = std::io::Error;

        async fn set_address(&self, _id: &Uuid, _address: &SocketAddr) -> Result<(), Self::Err> {
            Ok(())
        }

        async fn set_is_next(&self, _id: &Uuid, _is_next: bool) -> Result<(), Self::Err> {
            Ok(())
        }
    }

    // This test uses the kill signal to make sure a service does stop when asked to
    #[tokio::test]
    async fn can_be_killed() {
        let (built, storage_manager) = make_and_built("sleep-async");
        let id = built.id;
        let runtime_manager = get_runtime_manager();
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |response: Option<SubscribeStopResponse>| {
            let response = response.unwrap();
            match (
                StopReason::from_i32(response.reason).unwrap(),
                response.message,
            ) {
                (StopReason::Request, mes) if mes.is_empty() => cleanup_send.send(()).unwrap(),
                _ => panic!("expected stop due to request"),
            }
        };

        built
            .handle(
                storage_manager,
                StubSecretGetter,
                StubResourceManager,
                runtime_manager.clone(),
                StubDeploymentUpdater,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();

        // Give it some time to start up
        sleep(Duration::from_secs(1)).await;

        // Send kill signal
        assert!(runtime_manager.lock().await.kill(&id).await);

        tokio::select! {
            _ = sleep(Duration::from_secs(1)) => panic!("cleanup should have been called"),
            Ok(()) = cleanup_recv => {}
        }
    }

    // This test does not use a kill signal to stop the service. Rather the service decided to stop on its own without errors
    #[tokio::test]
    async fn self_stop() {
        let (built, storage_manager) = make_and_built("sleep-async");
        let runtime_manager = get_runtime_manager();
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |response: Option<SubscribeStopResponse>| {
            let response = response.unwrap();
            match (
                StopReason::from_i32(response.reason).unwrap(),
                response.message,
            ) {
                (StopReason::End, mes) if mes.is_empty() => cleanup_send.send(()).unwrap(),
                _ => panic!("expected stop due to self end"),
            }
        };

        built
            .handle(
                storage_manager,
                StubSecretGetter,
                StubResourceManager,
                runtime_manager.clone(),
                StubDeploymentUpdater,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service stopped on its own"),
            Ok(()) = cleanup_recv => {},
        }

        // Prevent the runtime manager from dropping earlier, which will kill the processes it manages
        drop(runtime_manager);
    }

    // Test for panics in Service::bind
    #[tokio::test]
    async fn panic_in_bind() {
        let (built, storage_manager) = make_and_built("bind-panic");
        let runtime_manager = get_runtime_manager();
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |response: Option<SubscribeStopResponse>| {
            let response = response.unwrap();
            match (
                StopReason::from_i32(response.reason).unwrap(),
                response.message,
            ) {
                (StopReason::Crash, mes) if mes.contains("panic in bind") => {
                    cleanup_send.send(()).unwrap()
                }
                (_, mes) => panic!("expected stop due to crash: {mes}"),
            }
        };

        built
            .handle(
                storage_manager,
                StubSecretGetter,
                StubResourceManager,
                runtime_manager.clone(),
                StubDeploymentUpdater,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service handle stopped after panic"),
            Ok(()) = cleanup_recv => {}
        }

        // Prevent the runtime manager from dropping earlier, which will kill the processes it manages
        drop(runtime_manager);
    }

    // Test for panics in the main function
    #[tokio::test]
    #[should_panic(expected = "Load(\"main panic\")")]
    async fn panic_in_main() {
        let (built, storage_manager) = make_and_built("main-panic");
        let runtime_manager = get_runtime_manager();

        let handle_cleanup = |_result| panic!("service should never be started");

        built
            .handle(
                storage_manager,
                StubSecretGetter,
                StubResourceManager,
                runtime_manager.clone(),
                StubDeploymentUpdater,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();
    }

    fn make_and_built(crate_name: &str) -> (Built, ArtifactsStorageManager) {
        let crate_dir: PathBuf = [RESOURCES_PATH, crate_name].iter().collect();

        Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&crate_dir)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        let lib_name = if cfg!(target_os = "windows") {
            format!("{}.exe", crate_name)
        } else {
            crate_name.to_string()
        };

        let id = Uuid::new_v4();
        let so_path = crate_dir.join("target/release").join(lib_name);
        let storage_manager = get_storage_manager();
        let new_so_path = storage_manager.deployment_executable_path(&id).unwrap();

        std::fs::copy(so_path, new_so_path).unwrap();
        (
            Built {
                id,
                service_name: crate_name.to_string(),
                service_id: Uuid::new_v4(),
                tracing_context: Default::default(),
                is_next: false,
                claim: None,
            },
            storage_manager,
        )
    }
}
