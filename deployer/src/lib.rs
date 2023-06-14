use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bollard::{Docker, API_DEFAULT_VERSION};
use chrono::Utc;
use deployment::persistence::{dal::Dal, Service};
use deployment::persistence::{Persistence, State};
use deployment::Deployment;
use error::{Error, Result};
use futures::TryFutureExt;
use http::Uri;
use project::docker::{ContainerSettings, ServiceDockerContext};
use project::driver::Run;
use project::service::state::a_creating::ServiceCreating;
use project::service::ServiceState;
use project::task::{BoxedTask, Task, TaskBuilder};
use runtime_manager::RuntimeManager;
use shuttle_common::backends::auth::VerifyClaim;
use shuttle_common::claims::Claim;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        tracing::ExtractPropagationLayer,
    },
    claims::Scope,
};
use shuttle_proto::deployer::{
    deployer_server::{Deployer, DeployerServer},
    DeployRequest, DeployResponse,
};
use tokio::sync::Mutex;
use tonic::transport::Endpoint;
use tonic::{transport::Server, Response, Result as TonicResult};
use tracing::{error, info};
use ulid::Ulid;

use crate::deployment::DeploymentManager;
use crate::project::task;
use crate::project::worker::{TaskRouter, Worker};

pub mod args;
pub mod deployment;
pub mod error;
pub mod project;
pub mod proxy;
pub mod runtime_manager;

pub struct DeployerService<D: Dal + Send + Sync + 'static> {
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    persistence: Persistence<D>,
    deployment_manager: DeploymentManager,
    bind_address: SocketAddr,
    docker: Docker,
    task_router: TaskRouter<BoxedTask>,
    provisioner_host: Endpoint,
    auth_uri: Uri,
    network_name: String,
    prefix: String,
    sender:
        tokio::sync::mpsc::Sender<Box<dyn Task<(), Output = (), Error = project::error::Error>>>,
    worker_handle: tokio::task::JoinHandle<std::result::Result<(), ()>>,
}

impl<D: Dal + Send + Sync + 'static> DeployerService<D> {
    pub async fn new(
        runtime_manager: Arc<Mutex<RuntimeManager>>,
        persistence: Persistence<D>,
        artifacts_path: PathBuf,
        bind_address: SocketAddr,
        docker_host: &str,
        provisioner_uri: Endpoint,
        auth_uri: Uri,
        network_name: String,
        prefix: String,
    ) -> Self {
        let deployment_manager = DeploymentManager::builder()
            .build_log_recorder(persistence.clone())
            .artifacts_path(artifacts_path)
            .runtime(runtime_manager.clone())
            .dal(persistence.dal().clone())
            .build();

        // We create the worker who will handle existing services refreshing.
        let worker = Worker::new();
        let sender: tokio::sync::mpsc::Sender<
            Box<dyn Task<(), Output = (), Error = project::error::Error>>,
        > = worker.sender();
        let worker_handle: tokio::task::JoinHandle<std::result::Result<(), ()>> = tokio::spawn(
            worker
                .start()
                .map_ok(|_| info!("worker terminated successfully"))
                .map_err(|err| error!("worker error: {}", err)),
        );

        Self {
            docker: Docker::connect_with_unix(docker_host, 60, API_DEFAULT_VERSION)
                .expect("to initialize docker connection the installed docker daemon"),
            runtime_manager,
            persistence,
            deployment_manager,
            bind_address,
            task_router: TaskRouter::default(),
            provisioner_host: provisioner_uri,
            auth_uri,
            network_name,
            prefix,
            worker_handle,
            sender,
        }
    }

    pub async fn start(self) -> Result<()> {
        // We update all the invalid deployments states to stopped.
        self.persistence
            .dal()
            .update_invalid_states_to_stopped()
            .await
            .expect("to have the invalid states stopped");

        // The deployments which are in the `Running` state are considered runnable and they are started again. Running the
        // deployments means we're loading and starting their associated entrypoints (the services are sandboxed in containers
        // that have an entrypoint with `shuttle-runtime`).
        let runnable_deployments = self.persistence.dal().running_deployments().await.unwrap();
        info!(count = %runnable_deployments.len(), "enqueuing runnable deployments");
        for existing_deployment in runnable_deployments {
            // We want to restart the corresponding deployment service container.
            let service = self
                .persistence
                .dal()
                .service(&existing_deployment.id)
                .await?;
            let image = match service.state.container() {
                Some(inner) => match inner.image {
                    Some(img) => img,
                    None => {
                        error!("can not get the container information because it's missing from the state");
                        continue;
                    }
                },
                None => {
                    error!(
                        "can not get the container information because it's missing from the state"
                    );
                    continue;
                }
            };

            let cs = ContainerSettings::builder()
                .image(image)
                .provisioner_host(self.provisioner_host.uri().to_string())
                .auth_uri(self.auth_uri.to_string())
                .network_name(self.network_name.to_string())
                .prefix(self.prefix.to_string())
                .build()
                .await;
            TaskBuilder::new(self.persistence.dal().clone())
                .task_router(self.task_router.clone())
                .service_id(service.id.clone())
                .service_context(ServiceDockerContext::new(
                    self.docker.clone(),
                    cs,
                    self.runtime_manager.clone(),
                ))
                .and_then(task::refresh())
                .and_then(task::run_until_done())
                .and_then(task::check_health())
                .send(&self.sender)
                .await
                .expect("to refresh old projects");

            let target_ip = match self
                .persistence
                .dal()
                .service(&service.id)
                .await?
                .state
                .container()
            {
                Some(inner) => match inner.network_settings {
                    Some(network) => match network.ip_address {
                        Some(ip) => ip
                            .parse::<Ipv4Addr>()
                            .expect("to have a valid IPv4 address"),
                        None => {
                            error!("ip address not found on the network setting of the service {} container", service.id);
                            continue;
                        }
                    },
                    None => {
                        error!(
                            "missing network settings on the service {} container",
                            service.id
                        );
                        continue;
                    }
                },
                None => {
                    error!(
                        "missing container inspect information for service {}",
                        service.id
                    );
                    continue;
                }
            };

            let built = Run {
                deployment_id: existing_deployment.id,
                service_name: existing_deployment.service_name,
                service_id: existing_deployment.service_id,
                tracing_context: Default::default(),
                is_next: existing_deployment.is_next,
                // We don't need a claim to be set to start existing running deployments.
                claim: None,
                target_ip,
            };
            self.deployment_manager.run_push(built).await;
        }

        let mut server_builder = Server::builder()
            .http2_keepalive_interval(Some(Duration::from_secs(60)))
            .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
                self.auth_uri.clone(),
            )))
            .layer(ExtractPropagationLayer);
        let bind_address = self.bind_address.clone();
        let svc = DeployerServer::new(self);
        let router = server_builder.add_service(svc);

        router
            .serve(bind_address)
            .await
            .expect("to serve on address");
        Ok(())
    }

    pub async fn push_deployment(
        &self,
        req: DeployRequest,
        state: ServiceState,
        claim: Option<Claim>,
    ) -> Result<String> {
        // Insert the service if not present.
        let service_id =
            Ulid::from_string(req.service_id.as_str()).map_err(|err| Error::UlidDecode(err))?;
        let service = Service {
            id: service_id.clone(),
            name: req.service_name.clone(),
            state_variant: state.to_string(),
            state,
        };
        self.persistence
            .dal()
            .insert_service_if_absent(service.clone())
            .await?;

        // Insert the new deployment.
        let deployment = Deployment {
            id: Ulid::new(),
            service_id: service.id,
            state: State::Built,
            last_update: Utc::now(),
            address: None,
            is_next: req.is_next,
            git_branch: Some(req.git_branch),
            git_commit_hash: Some(req.git_commit_hash),
            git_commit_message: Some(req.git_commit_message),
            git_dirty: Some(req.git_dirty),
        };
        self.persistence
            .dal()
            .insert_deployment(deployment.clone())
            .await?;

        Ok(deployment.id.to_string())
    }
}

#[async_trait]
impl<D: Dal + Send + Sync + 'static> Deployer for DeployerService<D> {
    async fn deploy(
        &self,
        request: tonic::Request<DeployRequest>,
    ) -> TonicResult<tonic::Response<DeployResponse>, tonic::Status> {
        request.verify(Scope::DeploymentPush)?;
        let claim = request.extensions().get::<Claim>().map(Clone::clone);
        let request = request.into_inner();
        let service_id: Ulid = Ulid::from_string(request.service_id.as_str())
            .map_err(|_| tonic::Status::invalid_argument("invalid service id"))?;

        // Check if there are running deployments for the service.
        let service_running_deployments = self
            .persistence
            .dal()
            .service_running_deployments(&service_id)
            .await
            .map_err(|_| tonic::Status::new(tonic::Code::Internal, "error triggered while checking the existing running deployments for the service"))?;
        if service_running_deployments.len() > 0 {
            return Err(tonic::Status::internal(
                "can not deploy due to existing running deployments",
            ));
        }

        // Propagate the failure as an internal error, with the message being the displayed error.
        // We're protected from bubling up sensitive info because the error thrown is a DalError
        // which is `Display` controlled.
        let state = ServiceState::Creating(ServiceCreating::new(
            request.service_id.clone(),
            u64::from(request.idle_minutes),
        ));
        let deployment_id = self
            .push_deployment(request.clone(), state.clone(), claim)
            .await
            .map_err(|err| tonic::Status::new(tonic::Code::Internal, err.to_string()))?;

        let creating = ServiceCreating::new(
            request.service_name.clone(),
            u64::from(request.idle_minutes),
        );
        let service_state = ServiceState::Creating(creating);

        // If the service already lives in the persistence.
        if let Some(state) = self
            .persistence
            .dal()
            .service_state(&service_id)
            .await
            .map_err(|_| tonic::Status::not_found("service not found"))?
        {
            // But is in the destroyed state.
            if state.is_destroyed() {
                // Recreate it.
                self.persistence
                    .dal()
                    .update_service_state(service_id, service_state)
                    .await
                    .map_err(|_| tonic::Status::invalid_argument("invalid service id"))?;
            } else {
                // Otherwise it already exists
                return Err(tonic::Status::already_exists(
                    "The service already exists in deployer persistence, skipping its creation.",
                ));
            }
        } else {
            // Insert the service.
            let service = Service {
                id: service_id,
                name: request.service_name,
                state_variant: state.to_string(),
                state,
            };
            self.persistence
                .dal()
                .insert_service_if_absent(service)
                .await
                .map_err(|_| {
                    tonic::Status::internal(
                        "failed because the service already exists or persisting it errored",
                    )
                })?;
        }

        let cs = ContainerSettings::builder()
            .image(request.image_name)
            .provisioner_host(self.provisioner_host.uri().to_string())
            .auth_uri(self.auth_uri.to_string())
            .network_name(self.network_name.to_string())
            .prefix(self.prefix.to_string())
            .build()
            .await;

        TaskBuilder::new(self.persistence.dal().clone())
            .service_id(service_id)
            .service_context(ServiceDockerContext::new(
                self.docker.clone(),
                cs,
                self.runtime_manager.clone(),
            ))
            .and_then(task::run_until_done())
            .and_then(task::check_health())
            .send(&self.sender)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        Ok(Response::new(DeployResponse { deployment_id }))
    }
}
