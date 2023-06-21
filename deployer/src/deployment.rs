use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use opentelemetry::global;
use portpicker::pick_unused_port;
use shuttle_common::claims::{Claim, ClaimService, InjectPropagation};

use shuttle_proto::runtime::{
    runtime_client::RuntimeClient, LoadRequest, StartRequest, StopReason, SubscribeStopRequest,
    SubscribeStopResponse,
};
use tokio::sync::mpsc;
use tonic::{transport::Channel, Code};
use tracing::{debug, debug_span, error, info, instrument, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;

use crate::{
    dal::Dal,
    project::service::{
        state::{
            c_starting::ServiceStarting, f_running::ServiceRunning, g_completed::ServiceCompleted,
            m_destroyed::ServiceDestroyed, m_errored::ServiceErrored, StateVariant,
        },
        ServiceState,
    },
    runtime_manager::RuntimeManager,
};

use crate::dal::DalError;

type RunReceiver = mpsc::Receiver<RunnableDeployment>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error")]
    Dal(DalError),
    #[error("Missing IPv4 address in the persistence")]
    MissingIpv4Address,
    #[error("Error occurred when running a deployment: {0}")]
    Send(String),
    #[error("Error at service runtime: {0}")]
    Runtime(anyhow::Error),
    #[error("Error preparing the service runtime: {0}")]
    PrepareRun(String),
    #[error("Encountered IO error: {0}")]
    IoError(std::io::Error),
    #[error("Error during the service load phase: {0}")]
    Load(String),
    #[error("Error during the service run phase: {0}")]
    Start(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Run a task which takes runnable deploys from a channel and starts them up on our runtime
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task<D: Dal + Sync + 'static>(
    dal: D,
    mut recv: RunReceiver,
    mut runtime_manager: RuntimeManager,
) {
    info!("Run task started");

    while let Some(runnable) = recv.recv().await {
        info!("Deployment to be run: {}", runnable.deployment_id);
        let runtime_client = runtime_manager
            .runtime_client(
                runnable.service_id,
                runnable
                    .target_ip
                    .expect("to have a target ip set for the runtime client"),
            )
            .await
            .expect("to set up a runtime client against a ready deployment");
        let claim = runnable.claim.clone();
        let runtime_manager = runtime_manager.clone();
        let dal = dal.clone();
        tokio::spawn(async move {
            let parent_cx = global::get_text_map_propagator(|propagator| {
                propagator.extract(&runnable.tracing_context)
            });
            let span = debug_span!("runner");
            span.set_parent(parent_cx);

            let deployment_id = runnable.deployment_id;

            // We are subscribing to the runtime logs emitted during the load phase here.
            if let Err(err) = runtime_manager
                .logs_subscribe(&runnable.service_id)
                .await
                .map_err(|err| Error::PrepareRun(err.to_string()))
            {
                start_crashed_cleanup(&deployment_id, err);
            }

            async move {
                if let Err(err) = runnable.load_and_run(dal, runtime_client, claim).await {
                    start_crashed_cleanup(&deployment_id, err)
                }

                info!("deployment done");
            }
            .instrument(span)
            .await
        });
    }
}

async fn cleanup<D: Dal + Sync + 'static>(
    response: Option<SubscribeStopResponse>,
    deployment_id: &Ulid,
    dal: D,
) {
    debug!(response = ?response,  "stop client response: ");

    if let Some(response) = response {
        match StopReason::from_i32(response.reason).unwrap_or_default() {
            StopReason::Request => stopped_cleanup(deployment_id),
            StopReason::End => completed_cleanup(deployment_id, dal).await.unwrap_or(()),
            StopReason::Crash => crashed_cleanup(
                deployment_id,
                Error::Runtime(anyhow::Error::msg(response.message)),
            ),
        }
    } else {
        crashed_cleanup(
            deployment_id,
            Error::Runtime(anyhow::anyhow!(
                "stop subscribe channel stopped unexpectedly"
            )),
        )
    }
}

#[instrument(skip(deployment_id, dal), fields(deployment_id = %deployment_id, state = %ServiceCompleted::name()))]
async fn completed_cleanup<D: Dal + Sync + 'static>(
    deployment_id: &Ulid,
    dal: D,
    // docker: Docker,
    // runtime_manager: RuntimeManager,
    // sender: tokio::sync::mpsc::Sender<
    //     Box<dyn Task<(), Output = (), Error = crate::project::error::Error>>,
    // >,
    // task_router: TaskRouter<BoxedTask>,
) -> Result<()> {
    info!("service was stopped by its own");

    // We're updating the service state. We should expect to have the container end by itself.
    let deployment = dal.deployment(deployment_id).await.map_err(Error::Dal)?;
    let service = dal
        .service(&deployment.service_id)
        .await
        .map_err(Error::Dal)?;
    if let Some(container) = service.state.container() {
        dal.update_service_state(
            service.id,
            ServiceState::Completed(
                ServiceCompleted::from_container(container)
                    .expect("to return a valid completed state"),
            ),
        )
        .await
        .map_err(Error::Dal)?;
    }

    Ok(())
}

#[instrument(skip(_id), fields(id = %_id, state = %ServiceDestroyed::name()))]
fn stopped_cleanup(_id: &Ulid) {
    // TODO: pass over here the necessary information to destroy the container.
    info!("service was stopped by the user");
}

#[instrument(skip(_id), fields(id = %_id, state = %ServiceErrored::name()))]
fn crashed_cleanup(_id: &Ulid, error: impl std::error::Error + 'static) {
    // TODO: pass over here the necessary information to remove any exisiting container.
    error!(
        error = &error as &dyn std::error::Error,
        "service encountered an error"
    );
}

#[instrument(skip(_id), fields(id = %_id, state = %ServiceErrored::name()))]
fn start_crashed_cleanup(_id: &Ulid, error: impl std::error::Error + 'static) {
    // TODO: pass over here the necessary information to remove any exisiting container.
    error!(
        error = &error as &dyn std::error::Error,
        "service startup encountered an error"
    );
}

#[derive(Clone, Debug)]
pub struct RunnableDeployment {
    pub deployment_id: Ulid,
    pub service_name: String,
    pub service_id: Ulid,
    pub tracing_context: HashMap<String, String>,
    // When deployments are reinstated at the deployment startup there is no claim.
    pub claim: Option<Claim>,
    // The target IP of the deployment, which is resolved after the service gets attached to the network.
    pub target_ip: Option<Ipv4Addr>,
    pub is_next: bool,
}

impl RunnableDeployment {
    #[instrument(skip(self, dal, runtime_client, claim), fields(id = %self.deployment_id, state = %ServiceStarting::name()))]
    #[allow(clippy::too_many_arguments)]
    async fn load_and_run<D: Dal + Sync + 'static>(
        self,
        dal: D,
        runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        claim: Option<Claim>,
    ) -> Result<()> {
        let port = match pick_unused_port() {
            Some(port) => port,
            None => {
                return Err(Error::PrepareRun(
                    "could not find a free port to deploy service on".to_string(),
                ))
            }
        };

        let address = SocketAddr::new(
            IpAddr::V4(self.target_ip.expect("to have a target ip set")),
            port,
        );

        // Execute loaded service
        load(self.service_name.clone(), runtime_client.clone(), claim).await?;

        tokio::spawn(run(self.deployment_id, dal, runtime_client, address));

        Ok(())
    }
}

async fn load(
    service_name: String,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    claim: Option<Claim>,
) -> Result<()> {
    // For alpha this is the path to the users project with an embedded runtime.
    // For shuttle-next this is the path to the compiled .wasm file, which will be
    // used in the load request.
    // TODO: we need pass down the path for shuttle next
    let path = String::new();
    let mut load_request = tonic::Request::new(LoadRequest {
        path,
        service_name,
        resources: Vec::new(),
        secrets: HashMap::new(),
    });

    if let Some(claim) = claim {
        load_request.extensions_mut().insert(claim);
    }

    debug!("loading service");
    let response = runtime_client.load(load_request).await;

    match response {
        Ok(response) => {
            let response = response.into_inner();

            info!(success = %response.success, "loading response");

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

#[instrument(skip(runtime_client, dal), fields(id=%id, state = %ServiceRunning::name()))]
async fn run<D: Dal + Sync + 'static>(
    id: Ulid,
    dal: D,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    address: SocketAddr,
) {
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

            cleanup(reason, &id, dal).await;
        }
        Err(ref status) if status.code() == Code::InvalidArgument => {
            cleanup(
                Some(SubscribeStopResponse {
                    reason: StopReason::Crash as i32,
                    message: status.to_string(),
                }),
                &id,
                dal,
            )
            .await;
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
    // use std::{
    //     net::{Ipv4Addr, SocketAddr},
    //     path::PathBuf,
    //     process::Command,
    //     sync::Arc,
    //     time::Duration,
    // };

    // use async_trait::async_trait;
    // use portpicker::pick_unused_port;
    // use shuttle_common::storage_manager::ArtifactsStorageManager;
    // use shuttle_proto::{
    //     provisioner::{
    //         provisioner_server::{Provisioner, ProvisionerServer},
    //         DatabaseDeletionResponse, DatabaseRequest, DatabaseResponse,
    //     },
    //     runtime::{StopReason, SubscribeStopResponse},
    // };
    // use tempfile::Builder;
    // use tokio::{
    //     sync::{oneshot, Mutex},
    //     time::sleep,
    // };
    // use tonic::transport::Server;
    // use uuid::Uuid;

    // use crate::{
    //     persistence::{DeploymentUpdater, Resource, ResourceManager, Secret, SecretGetter},
    //     RuntimeManager,
    // };

    // use super::Built;

    // const RESOURCES_PATH: &str = "tests/resources";

    // fn get_storage_manager() -> ArtifactsStorageManager {
    //     let tmp_dir = Builder::new().prefix("shuttle_run_test").tempdir().unwrap();
    //     let path = tmp_dir.into_path();

    //     ArtifactsStorageManager::new(path)
    // }

    // async fn kill_old_deployments() -> crate::error::Result<()> {
    //     Ok(())
    // }

    // struct ProvisionerMock;

    // #[async_trait]
    // impl Provisioner for ProvisionerMock {
    //     async fn provision_database(
    //         &self,
    //         _request: tonic::Request<DatabaseRequest>,
    //     ) -> Result<tonic::Response<DatabaseResponse>, tonic::Status> {
    //         panic!("no run tests should request a db");
    //     }

    //     async fn delete_database(
    //         &self,
    //         _request: tonic::Request<DatabaseRequest>,
    //     ) -> Result<tonic::Response<DatabaseDeletionResponse>, tonic::Status> {
    //         panic!("no run tests should delete a db");
    //     }
    // }

    // fn get_runtime_manager() -> Arc<Mutex<RuntimeManager>> {
    //     let provisioner_addr =
    //         SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
    //     let mock = ProvisionerMock;

    //     tokio::spawn(async move {
    //         Server::builder()
    //             .add_service(ProvisionerServer::new(mock))
    //             .serve(provisioner_addr)
    //             .await
    //             .unwrap();
    //     });

    //     let tmp_dir = Builder::new().prefix("shuttle_run_test").tempdir().unwrap();
    //     let path = tmp_dir.into_path();
    //     let (tx, rx) = crossbeam_channel::unbounded();

    //     tokio::runtime::Handle::current().spawn_blocking(move || {
    //         while let Ok(log) = rx.recv() {
    //             println!("test log: {log:?}");
    //         }
    //     });

    //     RuntimeManager::new(path, format!("http://{}", provisioner_addr), None, tx)
    // }

    // #[derive(Clone)]
    // struct StubSecretGetter;

    // #[async_trait]
    // impl SecretGetter for StubSecretGetter {
    //     type Err = std::io::Error;

    //     async fn get_secrets(&self, _service_id: &Uuid) -> Result<Vec<Secret>, Self::Err> {
    //         Ok(Default::default())
    //     }
    // }

    // #[derive(Clone)]
    // struct StubResourceManager;

    // #[async_trait]
    // impl ResourceManager for StubResourceManager {
    //     type Err = std::io::Error;

    //     async fn insert_resource(&self, _resource: &Resource) -> Result<(), Self::Err> {
    //         Ok(())
    //     }
    //     async fn get_resources(&self, _service_id: &Uuid) -> Result<Vec<Resource>, Self::Err> {
    //         Ok(Vec::new())
    //     }
    // }

    // #[derive(Clone)]
    // struct StubDeploymentUpdater;

    // #[async_trait]
    // impl DeploymentUpdater for StubDeploymentUpdater {
    //     type Err = std::io::Error;

    //     async fn set_address(&self, _id: &Uuid, _address: &SocketAddr) -> Result<(), Self::Err> {
    //         Ok(())
    //     }

    //     async fn set_is_next(&self, _id: &Uuid, _is_next: bool) -> Result<(), Self::Err> {
    //         Ok(())
    //     }
    // }

    // // This test uses the kill signal to make sure a service does stop when asked to
    // #[tokio::test]
    // async fn can_be_killed() {
    //     let (built, storage_manager) = make_and_built("sleep-async");
    //     let id = built.id;
    //     let runtime_manager = get_runtime_manager();
    //     let (cleanup_send, cleanup_recv) = oneshot::channel();

    //     let handle_cleanup = |response: Option<SubscribeStopResponse>| {
    //         let response = response.unwrap();
    //         match (
    //             StopReason::from_i32(response.reason).unwrap(),
    //             response.message,
    //         ) {
    //             (StopReason::Request, mes) if mes.is_empty() => cleanup_send.send(()).unwrap(),
    //             _ => panic!("expected stop due to request"),
    //         }
    //     };

    //     built
    //         .handle(
    //             storage_manager,
    //             StubSecretGetter,
    //             StubResourceManager,
    //             runtime_manager.clone(),
    //             StubDeploymentUpdater,
    //             kill_old_deployments(),
    //             handle_cleanup,
    //         )
    //         .await
    //         .unwrap();

    //     // Give it some time to start up
    //     sleep(Duration::from_secs(1)).await;

    //     // Send kill signal
    //     assert!(runtime_manager.lock().await.kill(&id).await);

    //     tokio::select! {
    //         _ = sleep(Duration::from_secs(1)) => panic!("cleanup should have been called"),
    //         Ok(()) = cleanup_recv => {}
    //     }
    // }

    // // This test does not use a kill signal to stop the service. Rather the service decided to stop on its own without errors
    // #[tokio::test]
    // async fn self_stop() {
    //     let (built, storage_manager) = make_and_built("sleep-async");
    //     let runtime_manager = get_runtime_manager();
    //     let (cleanup_send, cleanup_recv) = oneshot::channel();

    //     let handle_cleanup = |response: Option<SubscribeStopResponse>| {
    //         let response = response.unwrap();
    //         match (
    //             StopReason::from_i32(response.reason).unwrap(),
    //             response.message,
    //         ) {
    //             (StopReason::End, mes) if mes.is_empty() => cleanup_send.send(()).unwrap(),
    //             _ => panic!("expected stop due to self end"),
    //         }
    //     };

    //     built
    //         .handle(
    //             storage_manager,
    //             StubSecretGetter,
    //             StubResourceManager,
    //             runtime_manager.clone(),
    //             StubDeploymentUpdater,
    //             kill_old_deployments(),
    //             handle_cleanup,
    //         )
    //         .await
    //         .unwrap();

    //     tokio::select! {
    //         _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service stopped on its own"),
    //         Ok(()) = cleanup_recv => {},
    //     }

    //     // Prevent the runtime manager from dropping earlier, which will kill the processes it manages
    //     drop(runtime_manager);
    // }

    // // Test for panics in Service::bind
    // #[tokio::test]
    // async fn panic_in_bind() {
    //     let (built, storage_manager) = make_and_built("bind-panic");
    //     let runtime_manager = get_runtime_manager();
    //     let (cleanup_send, cleanup_recv) = oneshot::channel();

    //     let handle_cleanup = |response: Option<SubscribeStopResponse>| {
    //         let response = response.unwrap();
    //         match (
    //             StopReason::from_i32(response.reason).unwrap(),
    //             response.message,
    //         ) {
    //             (StopReason::Crash, mes) if mes.contains("panic in bind") => {
    //                 cleanup_send.send(()).unwrap()
    //             }
    //             (_, mes) => panic!("expected stop due to crash: {mes}"),
    //         }
    //     };

    //     built
    //         .handle(
    //             storage_manager,
    //             StubSecretGetter,
    //             StubResourceManager,
    //             runtime_manager.clone(),
    //             StubDeploymentUpdater,
    //             kill_old_deployments(),
    //             handle_cleanup,
    //         )
    //         .await
    //         .unwrap();

    //     tokio::select! {
    //         _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service handle stopped after panic"),
    //         Ok(()) = cleanup_recv => {}
    //     }

    //     // Prevent the runtime manager from dropping earlier, which will kill the processes it manages
    //     drop(runtime_manager);
    // }

    // // Test for panics in the main function
    // #[tokio::test]
    // #[should_panic(expected = "Load(\"main panic\")")]
    // async fn panic_in_main() {
    //     let (built, storage_manager) = make_and_built("main-panic");
    //     let runtime_manager = get_runtime_manager();

    //     let handle_cleanup = |_result| panic!("service should never be started");

    //     built
    //         .handle(
    //             storage_manager,
    //             StubSecretGetter,
    //             StubResourceManager,
    //             runtime_manager.clone(),
    //             StubDeploymentUpdater,
    //             kill_old_deployments(),
    //             handle_cleanup,
    //         )
    //         .await
    //         .unwrap();
    // }

    // fn make_and_built(crate_name: &str) -> (Built, ArtifactsStorageManager) {
    //     let crate_dir: PathBuf = [RESOURCES_PATH, crate_name].iter().collect();

    //     Command::new("cargo")
    //         .args(["build", "--release"])
    //         .current_dir(&crate_dir)
    //         .spawn()
    //         .unwrap()
    //         .wait()
    //         .unwrap();

    //     let lib_name = if cfg!(target_os = "windows") {
    //         format!("{}.exe", crate_name)
    //     } else {
    //         crate_name.to_string()
    //     };

    //     let id = Uuid::new_v4();
    //     let so_path = crate_dir.join("target/release").join(lib_name);
    //     let storage_manager = get_storage_manager();
    //     let new_so_path = storage_manager.deployment_executable_path(&id).unwrap();

    //     std::fs::copy(so_path, new_so_path).unwrap();
    //     (
    //         Built {
    //             id,
    //             service_name: crate_name.to_string(),
    //             service_id: Uuid::new_v4(),
    //             tracing_context: Default::default(),
    //             is_next: false,
    //             claim: None,
    //         },
    //         storage_manager,
    //     )
    // }
}
