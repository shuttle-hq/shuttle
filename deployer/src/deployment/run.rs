use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use opentelemetry::global;
use portpicker::pick_unused_port;
use shuttle_common::{
    claims::{Claim, ClaimService, InjectPropagation},
    constants::EXECUTABLE_DIRNAME,
    deployment::{
        DEPLOYER_END_MSG_COMPLETED, DEPLOYER_END_MSG_CRASHED, DEPLOYER_END_MSG_STARTUP_ERR,
        DEPLOYER_END_MSG_STOPPED, DEPLOYER_RUNTIME_START_RESPONSE,
    },
    resource,
};

use shuttle_proto::{
    resource_recorder::record_request,
    runtime::{
        runtime_client::RuntimeClient, LoadRequest, StartRequest, StopReason, SubscribeStopRequest,
        SubscribeStopResponse,
    },
};
use tokio::{
    sync::Mutex,
    task::{JoinHandle, JoinSet},
};
use tonic::{transport::Channel, Code};
use tracing::{debug, debug_span, error, info, instrument, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;
use uuid::Uuid;

use super::{RunReceiver, State};
use crate::{
    error::{Error, Result},
    persistence::{resource::ResourceManager, DeploymentUpdater},
    RuntimeManager,
};

/// Run a task which takes runnable deploys from a channel and starts them up on our runtime
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task(
    mut recv: RunReceiver,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    deployment_updater: impl DeploymentUpdater,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    resource_manager: impl ResourceManager,
    builds_path: PathBuf,
) {
    info!("Run task started");

    let mut set = JoinSet::new();

    loop {
        tokio::select! {
            Some(built) = recv.recv() => {
                let id = built.id;

                info!("Built deployment at the front of run queue: {id}");
                let deployment_updater = deployment_updater.clone();
                let resource_manager = resource_manager.clone();
                let builds_path = builds_path.clone();

                let old_deployments_killer = kill_old_deployments(
                    built.service_id,
                    id,
                    active_deployment_getter.clone(),
                    runtime_manager.clone(),
                );
                let runtime_manager_clone = runtime_manager.clone();
                let cleanup = move |response: Option<SubscribeStopResponse>| {
                    debug!(response = ?response,  "stop client response: ");

                    if let Some(response) = response {
                        match StopReason::try_from(response.reason).unwrap_or_default() {
                            StopReason::Request => stopped_cleanup(&id),
                            StopReason::End => completed_cleanup(&id),
                            StopReason::Crash => crashed_cleanup(
                                &id,
                                runtime_manager_clone,
                                Error::Run(anyhow::Error::msg(response.message).into()),
                            )
                        }
                    } else {
                        crashed_cleanup(
                            &id,
                            runtime_manager_clone,
                            Error::Runtime(anyhow::anyhow!(
                                "stop subscribe channel stopped unexpectedly"
                            )),
                        );
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
                                resource_manager,
                                runtime_manager,
                                deployment_updater,
                                old_deployments_killer,
                                cleanup,
                                builds_path.as_path(),
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
            },
            Some(res) = set.join_next() => {
                match res {
                    Ok(_) => (),
                    Err(err) => {
                        error!(error = %err, "an error happened while joining a deployment run task")
                    }
                }

            }
            else => break
        }
    }
}

#[instrument(skip(active_deployment_getter, deployment_id, runtime_manager))]
async fn kill_old_deployments(
    service_id: Ulid,
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
        info!("stopping old deployment (id {old_id})");

        if !guard.kill(&old_id).await {
            warn!("failed to kill old deployment (id {old_id})");
        }
    }

    Ok(())
}

#[instrument(name = "Cleaning up completed deployment", skip(_id), fields(deployment_id = %_id, state = %State::Completed))]
fn completed_cleanup(_id: &Uuid) {
    info!("{}", DEPLOYER_END_MSG_COMPLETED);
}

#[instrument(name = "Cleaning up stopped deployment", skip(_id), fields(deployment_id = %_id, state = %State::Stopped))]
fn stopped_cleanup(_id: &Uuid) {
    info!("{}", DEPLOYER_END_MSG_STOPPED);
}

#[instrument(name = "Cleaning up crashed deployment", skip(id, runtime_manager), fields(deployment_id = %id, state = %State::Crashed))]
fn crashed_cleanup(
    id: &Uuid,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    error: impl std::error::Error + 'static,
) {
    error!(
        error = &error as &dyn std::error::Error,
        "{}", DEPLOYER_END_MSG_CRASHED
    );

    // Fire a task which we'll not wait for. This initializes the runtime process killing.
    let id = *id;
    tokio::spawn(async move {
        runtime_manager.lock().await.kill_process(id);
    });
}

#[instrument(name = "Cleaning up startup crashed deployment", skip(_id), fields(deployment_id = %_id, state = %State::Crashed))]
fn start_crashed_cleanup(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "{}", DEPLOYER_END_MSG_STARTUP_ERR
    );
}

#[async_trait]
pub trait ActiveDeploymentsGetter: Clone + Send + Sync + 'static {
    type Err: std::error::Error + Send;

    async fn get_active_deployments(
        &self,
        service_id: &Ulid,
    ) -> std::result::Result<Vec<Uuid>, Self::Err>;
}

#[derive(Clone, Debug)]
pub struct Built {
    pub id: Uuid, // Deployment id
    pub service_name: String,
    pub service_id: Ulid,
    pub project_id: Ulid,
    pub tracing_context: HashMap<String, String>,
    pub is_next: bool,
    pub claim: Claim,
}

impl Built {
    #[instrument(
        name = "Loading resources",
        skip(self, resource_manager, runtime_manager, deployment_updater, kill_old_deployments, cleanup),
        fields(deployment_id = %self.id, state = %State::Loading)
    )]
    #[allow(clippy::too_many_arguments)]
    pub async fn handle(
        self,
        resource_manager: impl ResourceManager,
        runtime_manager: Arc<Mutex<RuntimeManager>>,
        deployment_updater: impl DeploymentUpdater,
        kill_old_deployments: impl futures::Future<Output = Result<()>>,
        cleanup: impl FnOnce(Option<SubscribeStopResponse>) + Send + 'static,
        builds_path: &Path,
    ) -> Result<JoinHandle<()>> {
        let project_path = builds_path.join(&self.service_name);
        // For alpha this is the path to the users project with an embedded runtime.
        // For shuttle-next this is the path to the compiled .wasm file, which will be
        // used in the load request.
        let executable_path = project_path
            .join(EXECUTABLE_DIRNAME)
            .join(self.id.to_string());

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
            .create_runtime_client(
                self.id,
                project_path.as_path(),
                self.service_name.clone(),
                alpha_runtime_path,
            )
            .await
            .map_err(Error::Runtime)?;

        kill_old_deployments.await?;
        // Execute loaded service
        load(
            self.service_name.clone(),
            self.service_id,
            executable_path.clone(),
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
    service_id: Ulid,
    executable_path: PathBuf,
    mut resource_manager: impl ResourceManager,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    claim: Claim,
) -> Result<()> {
    info!("Loading resources");

    let resources = resource_manager
        .get_resources(&service_id, claim.clone())
        .await
        .map_err(|err| Error::Load(err.to_string()))?
        .resources
        .into_iter()
        .map(resource::Response::try_from)
        // We ignore and trace the errors for resources with corrupted data, returning just the
        // valid resources.
        // TODO: investigate how the resource data can get corrupted.
        .filter_map(|resource| {
            resource
                .map_err(|err| {
                    error!(error = ?err, "failed to parse resource data");
                })
                .ok()
        })
        .map(resource::Response::into_bytes)
        .collect();

    let mut load_request = tonic::Request::new(LoadRequest {
        path: executable_path
            .into_os_string()
            .into_string()
            .unwrap_or_default(),
        service_name: service_name.clone(),
        resources,
        ..Default::default()
    });

    load_request.extensions_mut().insert(claim.clone());

    debug!(service_name = %service_name, "loading service");
    let response = runtime_client.load(load_request).await;

    debug!(service_name = %service_name, "service loaded");
    match response {
        Ok(response) => {
            let response = response.into_inner();
            // Make sure to not log the entire response, the resources field is likely to contain secrets.
            if response.success {
                info!("successfully loaded service");
            }

            let resources = response
                .resources
                .into_iter()
                .map(|res| {
                    let resource: resource::Response = serde_json::from_slice(&res).unwrap();
                    record_request::Resource {
                        r#type: resource.r#type.to_string(),
                        config: resource.config.to_string().into_bytes(),
                        data: resource.data.to_string().into_bytes(),
                    }
                })
                .collect();
            resource_manager
                .insert_resources(resources, &service_id, claim.clone())
                .await
                .expect("to add resource to persistence");

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

#[instrument(name = "Starting service", skip(runtime_client, deployment_updater, cleanup), fields(deployment_id = %id, state = %State::Running))]
async fn run(
    id: Uuid,
    service_name: String,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    address: SocketAddr,
    deployment_updater: impl DeploymentUpdater,
    cleanup: impl FnOnce(Option<SubscribeStopResponse>) + Send + 'static,
) {
    if let Err(err) = deployment_updater.set_address(&id, &address).await {
        // Clean up based on a stop response built outside the runtime
        cleanup(Some(SubscribeStopResponse {
            reason: StopReason::Crash as i32,
            message: format!("errored while setting the new deployer address: {}", err),
        }));
        return;
    }

    let start_request = tonic::Request::new(StartRequest {
        ip: address.to_string(),
    });

    // Subscribe to stop before starting to catch immediate errors
    let mut stream = match runtime_client
        .subscribe_stop(tonic::Request::new(SubscribeStopRequest {}))
        .await
    {
        Ok(stream) => stream.into_inner(),
        Err(err) => {
            // Clean up based on a stop response built outside the runtime
            cleanup(Some(SubscribeStopResponse {
                reason: StopReason::Crash as i32,
                message: format!("errored while opening the StopSubscribe channel: {}", err),
            }));
            return;
        }
    };

    let response = runtime_client.start(start_request).await;

    match response {
        Ok(response) => {
            if response.into_inner().success {
                info!("{}", DEPLOYER_RUNTIME_START_RESPONSE);
            }

            // Wait for stop reason
            match stream.message().await {
                Ok(reason) => cleanup(reason),
                // Stream closed abruptly, most probably runtime crashed.
                Err(err) => cleanup(Some(SubscribeStopResponse {
                    reason: StopReason::Crash as i32,
                    message: format!("runtime StopSubscribe channel errored: {}", err),
                })),
            }
        }
        Err(ref status) if status.code() == Code::InvalidArgument => {
            cleanup(Some(SubscribeStopResponse {
                reason: StopReason::Crash as i32,
                message: status.to_string(),
            }));
        }
        Err(ref status) => {
            error!(%status, "failed to start service");
            start_crashed_cleanup(
                &id,
                Error::Start("runtime failed to start deployment".to_string()),
            );
        }
    }
}
