use std::{
    collections::HashMap,
    future::Future,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use opentelemetry::global;
use serde::de::DeserializeOwned;
use shuttle_common::{
    claims::Claim,
    constants::{DEPLOYER_SERVICE_HTTP_PORT, EXECUTABLE_DIRNAME, RESOURCE_SCHEMA_VERSION},
    deployment::{
        DEPLOYER_END_MSG_COMPLETED, DEPLOYER_END_MSG_CRASHED, DEPLOYER_END_MSG_STARTUP_ERR,
        DEPLOYER_END_MSG_STOPPED, DEPLOYER_RUNTIME_START_FAILED, DEPLOYER_RUNTIME_START_RESPONSE,
    },
    resource::{self, ResourceInput, Type},
    DatabaseResource, DbInput, SecretStore,
};
use shuttle_proto::{
    provisioner::{self, DatabaseRequest},
    resource_recorder::record_request,
    runtime::{
        self, LoadRequest, StartRequest, StopReason, SubscribeStopRequest, SubscribeStopResponse,
    },
};
use shuttle_service::{Environment, ShuttleResourceOutput};
use tokio::{
    sync::Mutex,
    task::{JoinHandle, JoinSet},
};
use tonic::{Code, Request};
use tracing::{debug, debug_span, error, info, instrument, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;
use uuid::Uuid;

use super::{RunReceiver, State};
use crate::{
    error::{Error, Result},
    persistence::resource::ResourceManager,
    RuntimeManager,
};

/// Run a task which takes runnable deploys from a channel and starts them up on our runtime
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task(
    mut recv: RunReceiver,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    resource_manager: impl ResourceManager,
    builds_path: PathBuf,
    provisioner_client: provisioner::Client,
) {
    info!("Run task started");

    let mut set = JoinSet::new();

    loop {
        tokio::select! {
            Some(built) = recv.recv() => {
                let id = built.id;

                info!("Built deployment at the front of run queue: {id}");
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
                let provisioner_client = provisioner_client.clone();
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
                                old_deployments_killer,
                                cleanup,
                                builds_path.as_path(),
                                provisioner_client,
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
                        error!(
                            error = &err as &dyn std::error::Error,
                            "an error happened while joining a deployment run task"
                        )
                    }
                }

            }
            else => break
        }
    }
}

#[instrument(skip_all)]
async fn kill_old_deployments(
    service_id: Ulid,
    deployment_id: Uuid,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
) -> Result<()> {
    let mut guard = runtime_manager.lock().await;

    for old_id in active_deployment_getter
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
    /// must be set if this run will perform requests to backends
    pub claim: Option<Claim>,
    pub secrets: HashMap<String, String>,
}

impl Built {
    #[instrument(name = "Loading resources", skip_all, fields(deployment_id = %self.id, state = %State::Loading))]
    #[allow(clippy::too_many_arguments)]
    pub async fn handle(
        self,
        mut resource_manager: impl ResourceManager,
        runtime_manager: Arc<Mutex<RuntimeManager>>,
        kill_old_deployments: impl Future<Output = Result<()>>,
        cleanup: impl FnOnce(Option<SubscribeStopResponse>) + Send + 'static,
        builds_path: &Path,
        provisioner_client: provisioner::Client,
    ) -> Result<JoinHandle<()>> {
        let project_path = builds_path.join(&self.service_name);
        // This is the path to the users project with an embedded runtime.
        let executable_path = project_path
            .join(EXECUTABLE_DIRNAME)
            .join(self.id.to_string());
        let cached_resources_path = project_path
            .join(EXECUTABLE_DIRNAME)
            .join(format!("{}.resources", self.id));

        // Let the runtime expose its HTTP port
        let address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), DEPLOYER_SERVICE_HTTP_PORT);

        let runtime_client = runtime_manager
            .lock()
            .await
            .create_runtime_client(
                self.id,
                project_path.as_path(),
                self.service_name.clone(),
                executable_path,
            )
            .await
            .map_err(Error::Runtime)?;

        // Check for cached resources for this deployment id. This only succeeds on wakeup from idle or project restart.
        let resources = if let Some(bytes) = std::fs::read(&cached_resources_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(bytes.as_slice()).ok())
        {
            info!("Using cached resources");

            bytes
        }
        // Default case for handling resources and provisioning
        else {
            info!("Loading resources");
            let claim = self
                .claim
                .expect("claim must be present when loading resources");

            let mut new_secrets = self.secrets;
            let prev_resources = resource_manager
                .get_resources(&self.service_id, claim.clone())
                .await
                .map_err(|err| Error::Load(err.to_string()))?
                .resources
                .into_iter()
                .map(resource::Response::try_from)
                // Ignore and trace the errors for resources with corrupted data, returning just the valid resources.
                // TODO: investigate how the resource data can get corrupted.
                .filter_map(|resource| {
                    resource
                        .map_err(|err| {
                            error!(error = ?err, "failed to parse resource data");
                        })
                        .ok()
                })
                // inject old secrets into the secrets added in this deployment
                .inspect(|r| {
                    if r.r#type == shuttle_common::resource::Type::Secrets {
                        match serde_json::from_value::<SecretStore>(r.data.clone()) {
                            Ok(ss) => {
                                // Combine old and new, but insert old first so that new ones override.
                                let mut combined = HashMap::from_iter(ss.into_iter());
                                combined.extend(new_secrets.clone().into_iter());
                                new_secrets = combined;
                            }
                            Err(err) => {
                                error!(error = ?err, "failed to parse old secrets data");
                            }
                        }
                    }
                })
                .collect::<Vec<_>>();

            let resources = load(
                self.service_name.clone(),
                runtime_client.clone(),
                &new_secrets,
            )
            .await?;

            let resources = provision(
                self.service_name.as_str(),
                self.service_id,
                provisioner_client,
                resource_manager,
                claim,
                prev_resources,
                resources,
                new_secrets,
            )
            .await
            .map_err(Error::Provision)?;

            // cache the final resources output for use in wakeups
            // this should only happen on deployment, and not on wakeups
            std::fs::write(
                &cached_resources_path,
                serde_json::to_vec(&resources).expect("resources to serialize"),
            )
            .map_err(|_| Error::Load("Failed to save resource cache".into()))?;

            resources
        };

        kill_old_deployments.await?;

        let handler = tokio::spawn(run(
            self.id,
            self.service_name,
            runtime_client,
            address,
            cleanup,
            resources,
        ));

        Ok(handler)
    }
}

#[instrument(name = "Sending load request", skip_all)]
async fn load(
    service_name: String,
    mut runtime_client: runtime::Client,
    new_secrets: &HashMap<String, String>,
) -> Result<Vec<Vec<u8>>> {
    debug!(shuttle.project.name = %service_name, "loading service");
    let response = runtime_client
        .load(Request::new(LoadRequest {
            project_name: service_name.clone(),
            secrets: new_secrets.clone(),
            env: Environment::Deployment.to_string(),
            ..Default::default()
        }))
        .await;

    debug!(shuttle.project.name = %service_name, "service loaded");
    match response {
        Ok(response) => {
            let response = response.into_inner();
            // Make sure to not log the entire response, the resources field is likely to contain secrets.
            if response.success {
                info!("successfully loaded service");
            }

            if response.success {
                Ok(response.resources)
            } else {
                let error = Error::Load(response.message);
                error!(
                    error = &error as &dyn std::error::Error,
                    "failed to load service"
                );
                Err(error)
            }
        }
        Err(error) => {
            let error = Error::Load(error.to_string());
            error!(
                error = &error as &dyn std::error::Error,
                "failed to load service"
            );
            Err(error)
        }
    }
}

fn log(ty: &resource::Type, msg: &str) {
    info!("[Resource][{}] {}", ty, msg);
}

/// If an old resource with matching type + config and valid data exists, return it
fn get_cached_output<T: DeserializeOwned>(
    shuttle_resource_type: &Type,
    config: &serde_json::Value,
    prev_resources: &[resource::Response],
) -> Option<T> {
    prev_resources
        .iter()
        .find(|resource| resource.r#type == *shuttle_resource_type && resource.config == *config)
        .and_then(|resource| {
            let cached_output = resource.data.clone();
            log(shuttle_resource_type, "Found cached output");
            match serde_json::from_value::<T>(cached_output) {
                Ok(output) => Some(output),
                Err(_) => {
                    log(shuttle_resource_type, "Failed to validate cached output");
                    None
                }
            }
        })
}

#[instrument(name = "Provisioning resources", skip_all)]
#[allow(clippy::too_many_arguments)]
async fn provision(
    project_name: &str,
    service_id: Ulid,
    mut provisioner_client: provisioner::Client,
    mut resource_manager: impl ResourceManager,
    claim: Claim,
    prev_resources: Vec<resource::Response>,
    mut resources: Vec<Vec<u8>>,
    new_secrets: HashMap<String, String>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut resources_to_save: Vec<record_request::Resource> = Vec::new();

    // Fail early if any bytes is invalid json
    let values = resources
        .iter()
        .map(|bytes| {
            serde_json::from_slice::<ResourceInput>(bytes).context("deserializing resource input")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    for (bytes, shuttle_resource) in
        resources
            .iter_mut()
            .zip(values)
            // ignore non-Shuttle resource items
            .filter_map(|(bytes, value)| match value {
                ResourceInput::Shuttle(shuttle_resource) => Some((bytes, shuttle_resource)),
                ResourceInput::Custom(_) => None,
            })
            .map(|(bytes, shuttle_resource)| {
                if shuttle_resource.version == RESOURCE_SCHEMA_VERSION {
                    Ok((bytes, shuttle_resource))
                } else {
                    Err(anyhow!("
                        Shuttle resource request for {} with incompatible version found. Expected {}, found {}. \
                        Make sure that this deployer and the Shuttle resource are up to date.
                        ",
                        shuttle_resource.r#type,
                        RESOURCE_SCHEMA_VERSION,
                        shuttle_resource.version
                    ))
                }
            }).collect::<anyhow::Result<Vec<_>>>()?.into_iter()
    {
        // Mutate resource bytes with provisioning output if relevant

        // TODO (when moved to provisioner): Make the version integer be part of the cached output

        // Based on the resource type, do some of the following:
        //   - verify the related config struct (if one is expected)
        //   - provision the resource (if applicable)
        //   - add a mocked resource response to show to the user (if relevant)
        //   - overwrite the request's vec entry with the output of the provisioning (if provisioned)
        match shuttle_resource.r#type {
            resource::Type::Database(db_type) => {
                // no config fields are used yet, but verify the format anyways
                let config: DbInput = serde_json::from_value(shuttle_resource.config.clone())
                    .context("deserializing resource config")?;
                // We pass a Null config right now because this is relevant only for updating the resources
                // through the provisioner, which is something we don't support currently. If there will be
                // config fields that are relevant for provisioner updates on top of resources, they should
                // be cached.
                let output = get_cached_output(&shuttle_resource.r#type, &serde_json::Value::Null, prev_resources.as_slice());
                let output = match output {
                    Some(o) => o,
                    None => {
                        log(&shuttle_resource.r#type, "Provisioning...");
                        // ###
                        let mut req = Request::new(DatabaseRequest {
                            project_name: project_name.to_string(),
                            db_type: Some(db_type.into()),
                            db_name: config.db_name,
                            // other relevant config fields would go here
                        });
                        req.extensions_mut().insert(claim.clone());
                        let res = provisioner_client
                            .provision_database(req)
                            .await?
                            .into_inner();
                        DatabaseResource::Info(res.into())
                    }
                };

                // `output` is serialized as a `DatabaseInfo` due to being serde untagged
                resources_to_save.push(record_request::Resource {
                    r#type: shuttle_resource.r#type.to_string(),
                    // Send only the config fields that affect provisioning
                    // For now, this is "null" for all database types
                    config: serde_json::to_vec(&serde_json::Value::Null).expect("to serialize struct"),
                    data: serde_json::to_vec(&output).expect("to serialize struct"),
                });
                *bytes = serde_json::to_vec(&ShuttleResourceOutput {
                    output,
                    custom: shuttle_resource.custom,
                    state: None
                })
                .expect("to serialize struct");
            }
            resource::Type::Secrets => {
                // We already know the secrets at this stage, they are not provisioned like other resources
                resources_to_save.push(record_request::Resource {
                    r#type: shuttle_resource.r#type.to_string(),
                    config: serde_json::to_vec(&serde_json::Value::Null).expect("to serialize struct"),
                    data: serde_json::to_vec(&new_secrets).expect("to serialize struct"),
                });
                *bytes = serde_json::to_vec(&ShuttleResourceOutput {
                    output: new_secrets.clone(),
                    custom: shuttle_resource.custom,
                    state: None
                })
                .expect("to serialize struct");
            }
            resource::Type::Persist => {
                // this resource is still tracked until EOL, even though we don't provision it
                resources_to_save.push(record_request::Resource {
                    r#type: shuttle_resource.r#type.to_string(),
                    config: serde_json::to_vec(&serde_json::Value::Null).expect("to serialize struct"),
                    data: serde_json::to_vec(&serde_json::Value::Null).expect("to serialize struct"),
                });
            }
            resource::Type::Container => {
                bail!("Containers can't be requested during deployment");
            }
        }
    }

    // TODO: Move this to Provisioner and make it save after every resource is provisioned
    if resource_manager
        .insert_resources(resources_to_save, &service_id, claim.clone())
        .await
        .is_err()
    {
        bail!("failed saving resources to resource-recorder")
    }

    Ok(resources)
}

#[instrument(name = "Starting service", skip(runtime_client, cleanup, resources), fields(deployment_id = %id, state = %State::Running))]
async fn run(
    id: Uuid,
    service_name: String,
    mut runtime_client: runtime::Client,
    address: SocketAddr,
    cleanup: impl FnOnce(Option<SubscribeStopResponse>) + Send + 'static,
    resources: Vec<Vec<u8>>,
) {
    let start_request = tonic::Request::new(StartRequest {
        ip: address.to_string(),
        resources,
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
            } else {
                info!("{}", DEPLOYER_RUNTIME_START_FAILED);
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
            let error = Error::Start("runtime failed to start deployment".to_string());
            error!(
                %status,
                error = &error as &dyn std::error::Error,
                "failed to start service"
            );
            start_crashed_cleanup(&id, error);
        }
    }
}
