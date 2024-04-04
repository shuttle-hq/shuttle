use std::sync::Arc;

pub use persistence::Persistence;
pub use runtime_manager::RuntimeManager;
use shuttle_backends::client::ServicesApiClient;
use shuttle_common::log::LogRecorder;
use shuttle_proto::{logger, provisioner};
use tokio::sync::Mutex;
use tracing::info;
use ulid::Ulid;

mod args;
pub mod deployment;
pub mod error;
pub mod handlers;
pub mod persistence;
mod runtime_manager;

pub use crate::args::Args;
pub use crate::deployment::state_change_layer::StateChangeLayer;
use crate::deployment::{Built, DeploymentManager};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn start(
    persistence: Persistence,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    log_recorder: impl LogRecorder,
    log_fetcher: logger::Client,
    args: Args,
) {
    let project_id = Ulid::from_string(args.project_id.as_str())
        .expect("to have a valid ULID as project_id arg");

    // when _set is dropped once axum exits, the deployment tasks will be aborted.
    let deployment_manager = DeploymentManager::builder()
        .build_log_recorder(log_recorder)
        .active_deployment_getter(persistence.clone())
        .artifacts_path(args.artifacts_path)
        .runtime(runtime_manager)
        .resource_manager(persistence.clone())
        .provisioner_client(provisioner::get_client(args.provisioner_address).await)
        .queue_client(ServicesApiClient::new(args.gateway_uri))
        .log_fetcher(log_fetcher)
        .build();

    persistence.cleanup_invalid_states().await.unwrap();

    let deployments = persistence.get_all_runnable_deployments().await.unwrap();
    info!(count = %deployments.len(), "Deployments considered in the running state");
    // This works because they are returned in descending order.
    let mut deployments = deployments.into_iter();
    let last_running_deployment = deployments.next();
    info!("Marking all but last running deployment as stopped");
    for older_deployment in deployments {
        persistence
            .stop_running_deployment(older_deployment)
            .await
            .unwrap();
    }
    if let Some(deployment) = last_running_deployment {
        info!("Starting up last running deployment");
        let built = Built {
            id: deployment.id,
            service_name: deployment.service_name,
            service_id: deployment.service_id,
            project_id,
            tracing_context: Default::default(),
            claim: None,
            secrets: Default::default(),
        };
        deployment_manager.run_push(built).await;
    }

    let mut builder =
        handlers::RouterBuilder::new(persistence, deployment_manager, args.project, args.auth_uri);

    if args.local {
        // If the --local flag is passed, setup an auth layer in deployer
        builder = builder.with_local_admin_layer()
    } else {
        builder = builder.with_admin_secret_layer(args.admin_secret)
    };

    let router = builder.into_router();

    info!(address=%args.api_address, "Binding to and listening at address");

    axum::Server::bind(&args.api_address)
        .serve(router.into_make_service())
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", args.api_address));
}
