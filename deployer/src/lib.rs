use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use bollard::{Docker, API_DEFAULT_VERSION};
use chrono::Utc;
use deployment::persistence::{dal::Dal, Service};
use deployment::persistence::{Persistence, State};
use deployment::Deployment;
use derive_builder::Builder;
use error::{Error, Result};
use futures::task::AtomicWaker;
use futures::TryFutureExt;
use http::Uri;
use project::docker::{ContainerSettings, ServiceDockerContext};
use project::driver::DeploymentRun;
use project::service::state::a_creating::ServiceCreating;
use project::service::ServiceState;
use project::task::{BoxedTask, Task, TaskBuilder};
use runtime_manager::RuntimeManager;
use shuttle_common::backends::auth::VerifyClaim;
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
pub mod runtime_manager;

#[derive(Builder, Clone)]
pub struct DeployerServiceConfig {
    bind_address: SocketAddr,
    docker_host: PathBuf,
    provisioner_uri: Uri,
    auth_uri: Uri,
    network_name: String,
    prefix: String,
    artifacts_path: PathBuf,
}

pub struct DeployerService<D: Dal + Send + Sync + 'static> {
    deployment_manager: DeploymentManager<D>,
    runtime_manager: RuntimeManager,
    docker: Docker,
    persistence: Persistence<D>,
    task_router: TaskRouter<BoxedTask>,
    sender:
        tokio::sync::mpsc::Sender<Box<dyn Task<(), Output = (), Error = project::error::Error>>>,
    config: DeployerServiceConfig,
}

impl<D: Dal + Send + Sync + 'static> DeployerService<D> {
    pub async fn new(
        runtime_manager: RuntimeManager,
        persistence: Persistence<D>,
        config: DeployerServiceConfig,
    ) -> Self {
        let deployment_manager = DeploymentManager::builder()
            .build_log_recorder(persistence.clone())
            .artifacts_path(config.artifacts_path.clone())
            .runtime(runtime_manager.clone())
            .dal(persistence.dal().clone())
            .build();

        // We create the worker who handles creation of workers per service.
        // We're sending through this channel the work that needs to be taken
        // care of for a service.
        let worker = Worker::new();
        let sender: tokio::sync::mpsc::Sender<
            Box<dyn Task<(), Output = (), Error = project::error::Error>>,
        > = worker.sender();
        tokio::spawn(
            worker
                .start()
                .map_ok(|_| info!("worker terminated successfully"))
                .map_err(|err| error!("worker error: {}", err)),
        );

        Self {
            docker: Docker::connect_with_unix(
                config
                    .docker_host
                    .to_str()
                    .expect("docker host path to be a valid filesystem path"),
                60,
                API_DEFAULT_VERSION,
            )
            .expect("to initialize docker connection the installed docker daemon"),
            runtime_manager,
            persistence,
            deployment_manager,
            task_router: TaskRouter::default(),
            sender,
            config,
        }
    }

    pub async fn start(self) -> Result<()> {
        // First we update all the invalid deployments states to stopped.
        self.persistence
            .dal()
            .update_invalid_states_to_stopped()
            .await
            .expect("to have the invalid states stopped");

        // The deployments which are in the `Running` state are considered runnable and they are started again. Running the
        // deployments happens after their associated services' sandboxes are healthy and we start them.
        let runnable_deployments = self.persistence.dal().running_deployments().await.unwrap();
        info!(count = %runnable_deployments.len(), "enqueuing runnable deployments");
        for existing_deployment in runnable_deployments {
            // We want to restart the corresponding deployment service container.
            let image_name = self
                .persistence
                .dal()
                .service(&existing_deployment.service_id)
                .await?
                .state
                .image()
                .map_err(|err| Error::Internal(err.to_string()))?;

            // Start the sandbox and run the shuttle-runtime in a separate task.
            // let start_sandbox = self.start_sandbox(image_name, existing_deployment.service_id);
            let provisioner_uri = self.config.provisioner_uri.to_string();
            let auth_uri = self.config.auth_uri.to_string();
            let network_name = self.config.network_name.clone();
            let prefix = self.config.prefix.clone();
            let dal_clone = self.persistence.dal().clone();
            let task_router_clone = self.task_router.clone();
            let deployment_manager_clone = self.deployment_manager.clone();
            let docker_clone = self.docker.clone();
            let runtime_manager = self.runtime_manager.clone();
            let sender_clone = self.sender.clone();
            tokio::spawn(async move {
                // Refresh the docker containers for the old running deployments. This doesn't start
                // the services' runtimes yet.
                let cs = ContainerSettings::builder()
                    .image(image_name)
                    .provisioner_host(provisioner_uri)
                    .auth_uri(auth_uri)
                    .network_name(network_name.clone())
                    .prefix(prefix)
                    .build()
                    .await;

                // Awaiting on the task handle waits for the check_health to pass.
                TaskBuilder::new(dal_clone)
                    .task_router(task_router_clone)
                    .service_id(existing_deployment.service_id)
                    .service_context(ServiceDockerContext::new(docker_clone, cs, runtime_manager))
                    .and_then(task::refresh())
                    .and_then(task::run_until_done())
                    .and_then(task::check_health())
                    .send(&sender_clone)
                    .await
                    .expect("to get a handle of the created task")
                    .await;
                deployment_manager_clone
                    .run_deployment(
                        existing_deployment.service_id,
                        existing_deployment.id,
                        network_name.as_str(),
                        None,
                    )
                    .await
            });
        }

        let mut server_builder = Server::builder()
            .http2_keepalive_interval(Some(Duration::from_secs(60)))
            .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
                self.config.auth_uri.clone(),
            )))
            .layer(ExtractPropagationLayer);
        let bind_address = self.config.bind_address;
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
    ) -> Result<Deployment> {
        let service_id: Ulid =
            Ulid::from_string(req.service_id.as_str()).map_err(Error::UlidDecode)?;
        // If the service already lives in the persistence.
        if let Some(state) = self
            .persistence
            .dal()
            .service_state(&service_id)
            .await
            .map_err(Error::Dal)?
        {
            // But is in the destroyed state.
            info!("{}", state);
            if state.is_destroyed() {
                // Recreate it.
                self.persistence
                    .dal()
                    .update_service_state(service_id, state)
                    .await
                    .map_err(Error::Dal)?;
            } else {
                // Otherwise it already exists
                return Err(Error::ServiceAlreadyExists);
            }
        } else {
            // Insert the service.
            let service = Service {
                id: service_id,
                name: req.service_name,
                state_variant: state.to_string(),
                state,
            };
            self.persistence
                .dal()
                .insert_service_if_absent(service)
                .await
                .map_err(Error::Dal)?;
        }

        // Insert the new deployment.
        let deployment = Deployment {
            id: Ulid::new(),
            service_id,
            state: State::Built,
            last_update: Utc::now(),
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

        Ok(deployment)
    }
}

#[async_trait]
impl<D: Dal + Send + Sync + 'static> Deployer for DeployerService<D> {
    async fn deploy(
        &self,
        request: tonic::Request<DeployRequest>,
    ) -> TonicResult<tonic::Response<DeployResponse>, tonic::Status> {
        // Authorize the request.
        // request.verify(Scope::DeploymentPush)?;
        let request = request.into_inner();
        let service_id: Ulid = Ulid::from_string(request.service_id.as_str())
            .map_err(|_| tonic::Status::invalid_argument("invalid service id"))?;

        // Check if there are running deployments for the service.
        // TODO: we might need to not check running deployments because we
        // should be able to support one runtime that is loaded and one runtime
        // the runs.
        let service_running_deployments = self
            .persistence
            .dal()
            .service_running_deployments(&service_id)
            .await
            .map_err(|_| tonic::Status::new(tonic::Code::Internal, "error triggered while checking the existing running deployments for the service"))?;
        if !service_running_deployments.is_empty() {
            return Err(tonic::Status::internal(
                "can not deploy due to existing running deployments",
            ));
        }

        // Create a new deployment for the service and update the service state.
        let state = ServiceState::Creating(ServiceCreating::new(
            request.service_id.clone(),
            u64::from(request.idle_minutes),
        ));
        let deployment = self
            .push_deployment(request.clone(), state.clone())
            .await
            .map_err(|err| tonic::Status::new(tonic::Code::Internal, err.to_string()))?;

        // Start the sandbox and run the shuttle-runtime in a separate task.
        // let start_sandbox = self.start_sandbox(image_name, existing_deployment.service_id);
        let provisioner_uri = self.config.provisioner_uri.to_string();
        let auth_uri = self.config.auth_uri.to_string();
        let network_name = self.config.network_name.clone();
        let prefix = self.config.prefix.clone();
        let dal_clone = self.persistence.dal().clone();
        let task_router_clone = self.task_router.clone();
        let deployment_manager_clone = self.deployment_manager.clone();
        let docker_clone = self.docker.clone();
        let runtime_manager = self.runtime_manager.clone();
        let sender_clone = self.sender.clone();
        let deployment_id = Ulid::new();
        tokio::spawn(async move {
            // Refresh the docker containers for the old running deployments. This doesn't start
            // the services' runtimes yet.
            let cs = ContainerSettings::builder()
                .image(request.image_name)
                .provisioner_host(provisioner_uri)
                .auth_uri(auth_uri)
                .network_name(network_name.clone())
                .prefix(prefix)
                .build()
                .await;

            // Awaiting on the task handle waits for the check_health to pass.
            TaskBuilder::new(dal_clone)
                .task_router(task_router_clone)
                .service_id(service_id)
                .service_context(ServiceDockerContext::new(docker_clone, cs, runtime_manager))
                .and_then(task::refresh())
                .and_then(task::run_until_done())
                .and_then(task::check_health())
                .send(&sender_clone)
                .await
                .expect("to get a handle of the created task")
                .await;
            deployment_manager_clone
                .run_deployment(service_id, deployment_id, network_name.as_str(), None)
                .await
        });

        Ok(Response::new(DeployResponse {
            deployment_id: deployment_id.to_string(),
        }))
    }
}
