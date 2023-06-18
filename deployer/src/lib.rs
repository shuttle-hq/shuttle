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
use futures::TryFutureExt;
use http::Uri;
use project::docker::{ContainerSettings, ServiceDockerContext};
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
    DeployRequest, DeployResponse, Deployment as ProtoDeployment,
};
use shuttle_proto::deployer::{
    RestartDeploymentRequest, RestartDeploymentResponse, StopDeploymentRequest,
};
use tonic::{transport::Server, Response, Result as TonicResult};
use tracing::{debug, error, info, instrument};
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
        let runnable_deployments = self.persistence.dal().running_deployments().await?;
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
            // Clean the previous docker container if any.
            self.instate_service(
                &existing_deployment.service_id,
                existing_deployment.service_name,
                image_name.clone(),
                existing_deployment.idle_minutes,
                true,
            )
            .await?;

            self.instate_deployment(
                image_name,
                existing_deployment.service_id,
                existing_deployment.id,
                // We don't need a claim to start again an existing running deployment.
                None,
                existing_deployment.is_next,
            )
            .await;
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

    pub async fn instate_service(
        &self,
        service_id: &Ulid,
        service_name: String,
        image_name: String,
        idle_minutes: u64,
        overwrite: bool,
    ) -> Result<()> {
        let creating = ServiceState::Creating(ServiceCreating::new(
            service_id.to_string(),
            image_name,
            idle_minutes,
        ));
        // If the service already lives in the persistence with a previous state.
        if let Some(state) = self
            .persistence
            .dal()
            .service_state(service_id)
            .await
            .map_err(Error::Dal)?
        {
            // But is in the destroyed state.
            if state.is_destroyed() {
                // Update the state to creating, to reinstate it.
                self.persistence
                    .dal()
                    .update_service_state(*service_id, creating)
                    .await
                    .map_err(Error::Dal)?;
            } else if overwrite {
                // When overwritting, we must make sure we're transitioning to a new state
                // on clean. This is done by destroying the previous deployment.
                let dal = self.persistence.dal().clone();
                let task_router = self.task_router.clone();
                let docker = self.docker.clone();
                let runtime_manager = self.runtime_manager.clone();
                let sender = self.sender.clone();
                let service_id = *service_id;

                // Destroy the existing service sandbox.
                TaskBuilder::new(dal)
                    .task_router(task_router)
                    .service_id(service_id)
                    .service_context(ServiceDockerContext::new(
                        docker,
                        // Destroying an existing container
                        ContainerSettings::default(),
                        runtime_manager,
                    ))
                    .and_then(task::destroy())
                    .and_then(task::run_until_done())
                    .send(&sender)
                    .await
                    .expect("to get a handle of the created task")
                    .await;

                // Update the service with the creating state.
                self.persistence
                    .dal()
                    .update_service_state(service_id, creating)
                    .await
                    .map_err(Error::Dal)?;
            } else {
                // Otherwise it already exists
                return Err(Error::ServiceAlreadyExists);
            }
        } else {
            // Insert the service.
            let service = Service {
                id: *service_id,
                name: service_name,
                state_variant: creating.to_string(),
                state: creating,
            };

            self.persistence
                .dal()
                .insert_service_if_absent(service)
                .await
                .map_err(Error::Dal)?;
        }

        Ok(())
    }

    pub async fn create_deployment(&self, req_deployment: ProtoDeployment) -> Result<Deployment> {
        // Insert the new deployment.
        let service_id: Ulid =
            Ulid::from_string(req_deployment.service_id.as_str()).map_err(Error::UlidDecode)?;
        let deployment = Deployment {
            id: Ulid::new(),
            service_id,
            state: State::Built,
            last_update: Utc::now(),
            is_next: req_deployment.is_next,
            git_branch: req_deployment.git_branch,
            git_commit_hash: req_deployment.git_commit_hash,
            git_commit_message: req_deployment.git_commit_message,
            git_dirty: req_deployment.git_dirty,
        };
        self.persistence
            .dal()
            .insert_deployment(deployment.clone())
            .await?;
        debug!("created deployment");
        Ok(deployment)
    }

    // Establish a deployment based on the state found in persistence and based on
    // the deployer configuration data.
    async fn instate_deployment(
        &self,
        image_name: String,
        service_id: Ulid,
        deployment_id: Ulid,
        claim: Option<Claim>,
        is_next: bool,
    ) {
        // Start the deplyoment sandbox and run the shuttle-runtime in a separate task.
        let provisioner_uri = self.config.provisioner_uri.to_string();
        let auth_uri = self.config.auth_uri.to_string();
        let network_name = self.config.network_name.clone();
        let prefix = self.config.prefix.clone();
        let dal = self.persistence.dal().clone();
        let task_router = self.task_router.clone();
        let deployment_manager = self.deployment_manager.clone();
        let docker = self.docker.clone();
        let runtime_manager = self.runtime_manager.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let cs = ContainerSettings::builder()
                .image(image_name)
                .provisioner_host(provisioner_uri)
                .auth_uri(auth_uri)
                .network_name(network_name.clone())
                .prefix(prefix)
                .build()
                .await;
            debug!("driving the task for the docker container creation");
            // Awaiting on the task handle waits for the check_health to pass.
            TaskBuilder::new(dal)
                .task_router(task_router)
                .service_id(service_id)
                .service_context(ServiceDockerContext::new(docker, cs, runtime_manager))
                .and_then(task::run_until_done())
                .and_then(task::check_health())
                .send(&sender)
                .await
                .expect("to get a handle of the created task")
                .await;

            deployment_manager
                .run_deployment(
                    service_id,
                    deployment_id,
                    network_name.as_str(),
                    claim,
                    is_next,
                )
                .await
        });
    }
}

#[async_trait]
impl<D: Dal + Sync + 'static> Deployer for DeployerService<D> {
    #[instrument(skip(self, request))]
    async fn deploy(
        &self,
        request: tonic::Request<DeployRequest>,
    ) -> TonicResult<tonic::Response<DeployResponse>, tonic::Status> {
        // Authorize the request.
        request.verify(Scope::DeploymentPush)?;

        let claim = request.extensions().get::<Claim>().cloned();
        let request = request.into_inner();
        let req_deployment = request
            .deployment
            .ok_or(tonic::Status::invalid_argument("deployment info missing"))?;
        let service_id: Ulid = Ulid::from_string(req_deployment.service_id.as_str())
            .map_err(|_| tonic::Status::invalid_argument("invalid service id"))?;

        // Instate the service in the creating state.
        self.instate_service(
            &service_id,
            req_deployment.service_name.clone(),
            req_deployment.image_name.clone(),
            u64::from(req_deployment.idle_minutes),
            false,
        )
        .await
        .map_err(|err| match err {
            Error::ServiceAlreadyExists => tonic::Status::already_exists(err.to_string()),
            _ => tonic::Status::internal(err.to_string()),
        })?;

        // Create the deployment.
        let is_next = req_deployment.is_next;
        let image_name = req_deployment.image_name.clone();
        let deployment = self
            .create_deployment(req_deployment)
            .await
            .map_err(|err| tonic::Status::new(tonic::Code::Internal, err.to_string()))?;

        self.instate_deployment(image_name, service_id, deployment.id, claim, is_next)
            .await;

        Ok(Response::new(DeployResponse {
            deployment_id: deployment.id.to_string(),
        }))
    }
}
