use std::sync::Arc;

pub use persistence::Persistence;
pub use runtime_manager::RuntimeManager;
use shuttle_common::log::LogRecorder;
use shuttle_proto::{builder, logger};
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
use crate::deployment::DeploymentManager;
use shuttle_common::backends::client::gateway;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn start(
    persistence: Persistence,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    log_recorder: impl LogRecorder,
    log_fetcher: logger::Client,
    builder_client: Option<builder::Client>,
    args: Args,
) {
    // when _set is dropped once axum exits, the deployment tasks will be aborted.
    let deployment_manager = DeploymentManager::builder()
        .build_log_recorder(log_recorder)
        .active_deployment_getter(persistence.clone())
        .artifacts_path(args.artifacts_path)
        .runtime(runtime_manager)
        .deployment_updater(persistence.clone())
        .resource_manager(persistence.clone())
        .builder_client(builder_client)
        .queue_client(gateway::Client::new(
            args.gateway_uri.clone(),
            args.gateway_uri,
        ))
        .log_fetcher(log_fetcher)
        .build();

    persistence.cleanup_invalid_states().await.unwrap();

    let runnable_deployments = persistence.get_all_runnable_deployments().await.unwrap();
    info!(count = %runnable_deployments.len(), "stopping all but last running deploy");

    // Make sure we don't stop the last running deploy. This works because they are returned in descending order.
    let project_id = Ulid::from_string(args.project_id.as_str())
        .expect("to have a valid ULID as project_id arg");
    for existing_deployment in runnable_deployments.into_iter().skip(1) {
        persistence
            .stop_running_deployment(existing_deployment)
            .await
            .unwrap();
    }

    let mut builder = handlers::RouterBuilder::new(
        persistence,
        deployment_manager,
        args.project,
        project_id,
        args.auth_uri,
    );

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
