use async_trait::async_trait;
use chrono::{Duration, Utc};
use deployment::persistence::{dal::Dal, Service};
use deployment::persistence::{Persistence, State};
use deployment::Deployment;
use error::Result;
use http::Uri;
use project::driver::Built;
use project::service::state::creating::ServiceCreating;
use project::service::ServiceState;
use runtime_manager::RuntimeManager;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer, VerifyClaim},
        tracing::ExtractPropagationLayer,
    },
    claims::Scope,
};
use shuttle_proto::deployer::{
    deployer_server::{Deployer, DeployerServer},
    DeployRequest, DeployResponse,
};
use sqlx::types::Json as SqlxJson;
use tonic::{transport::Server, Response, Result as TonicResult};
use tracing::info;
use ulid::Ulid;

use crate::deployment::{gateway_client::GatewayClient, DeploymentManager};

pub mod account;
pub mod args;
pub mod deployment;
pub mod error;
pub mod project;
pub mod proxy;
pub mod runtime_manager;

pub struct DeployerService<D: Dal + Send + Sync + 'static> {
    runtime_manager: RuntimeManager,
    persistence: Persistence<D>,
    deployment_manager: DeploymentManager,
    gateway_uri: Uri,
    auth_uri: Uri,
    deployer_bind_address: Uri,
}

impl<D: Dal + Send + Sync + 'static> DeployerService<D> {
    pub async fn new(
        runtime_manager: RuntimeManager,
        persistence: Persistence<D>,
        gateway_uri: Uri,
        auth_uri: Uri,
        deployer_bind_address: Uri,
    ) -> Self {
        let deployment_manager = DeploymentManager::builder()
            .build_log_recorder(persistence.clone())
            .secret_recorder(persistence.clone())
            .active_deployment_getter(persistence.clone())
            .artifacts_path(runtime_manager.artifacts_path().clone())
            .runtime(runtime_manager.clone())
            .deployment_updater(persistence.clone())
            .secret_getter(persistence.clone())
            .queue_client(GatewayClient::new(gateway_uri.clone()))
            .build();
        Self {
            runtime_manager,
            persistence,
            deployment_manager,
            gateway_uri,
            auth_uri,
            deployer_bind_address,
        }
    }

    pub async fn start(self) -> Result<()> {
        self.persistence
            .dal()
            .update_invalid_states_to_stopped()
            .await
            .unwrap();

        let runnable_deployments = self.persistence.dal().runnable_deployments().await.unwrap();
        info!(count = %runnable_deployments.len(), "enqueuing runnable deployments");
        for existing_deployment in runnable_deployments {
            let built = Built {
                id: existing_deployment.id,
                service_name: existing_deployment.service_name,
                service_id: existing_deployment.service_id,
                tracing_context: Default::default(),
                is_next: existing_deployment.is_next,
            };
            self.deployment_manager.run_push(built).await;
        }

        let mut server_builder = Server::builder()
            .http2_keepalive_interval(Some(Duration::from_secs(60)))
            .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
                self.auth_uri,
            )))
            .layer(ExtractPropagationLayer);
        let svc = DeployerServer::new(self);
        let router = server_builder.add_service(svc);

        router
            .serve(self.deployer_bind_address)
            .await
            .expect("to serve on address")
    }

    pub async fn push_deployment(&self, req: DeployRequest, state: ServiceState) -> Result<String> {
        // Insert the service if not present.
        let service = Service {
            id: Ulid::from(req.service_id),
            name: req.service_name,
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

        // TODO: We assume the deploy request refers to a service which is already built. We will
        // confront this assumption later on in the stack by downloading an image from ECR. It
        // would've been best to have a fail fast mechanism called in this scope.
        let built = Built {
            id: deployment.id,
            service_name: req.service_name,
            service_id: Ulid::from(req.service_id),
            tracing_context: Default::default(),
            is_next: req.is_next,
        };
        self.deployment_manager.run_push(built).await;

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
        let request = request.into_inner();
        // Propagate the failure as an internal error, with the message being the displayed error.
        // We're protected from bubling up sensitive info because the error thrown is a DalError
        // which is `Display` controlled.
        let state = SqlxJson(ServiceState::Creating(ServiceCreating::new(
            request.service_id,
            u64::from(request.idle_minutes),
        )));
        let deployment_id = self
            .push_deployment(request, state)
            .await
            .map_err(|err| tonic::Status::new(tonic::Code::Internal, err.to_string()))?;
        let service_id: Ulid = Ulid::from_string(request.service_id.as_str())
            .map_err(tonic::Status::invalid_argument("Invalid service id."))?;

        // If the service already lives in the persistence.
        if let Some(state) = self.persistence.dal().service_state().await? {
            // But is in the destroyed state.
            if state.is_destroyed() {
                // Recreate it.
                let mut creating = ServiceCreating::new_with_random_initial_key(
                    request.service_name,
                    request.idle_minutes,
                );

                let project = ServiceState::Creating(creating);
                self.persistence
                    .dal()
                    .update_service_state(&service_id, state)
                    .await?;
                Ok(project)
            } else {
                // Otherwise it already exists
                Err(tonic::Status::already_exists(
                    "The service already exists in deployer persistence, skipping its creation.",
                ))
            }
        } else {
            // Insert the service.
            self.persistence
                .dal()
                .insert_service_if_absent(Service {
                    id: &service_id,
                    name: "".to_string(),
                    state_variant: state.to_string(),
                    state,
                })
                .await?;
        }
        Ok(Response::new(DeployResponse { deployment_id }))
    }
}
