use std::{convert::Infallible, net::SocketAddr, sync::Arc};

use fqdn::FQDN;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
};
pub use persistence::Persistence;
use proxy::AddressGetter;
pub use runtime_manager::RuntimeManager;
use shuttle_common::log::LogRecorder;
use shuttle_proto::{builder::builder_client::BuilderClient, logger::logger_client::LoggerClient};
use tokio::sync::Mutex;
use tracing::{error, info};
use ulid::Ulid;

mod args;
pub mod deployment;
pub mod error;
pub mod handlers;
pub mod persistence;
mod proxy;
mod runtime_manager;

pub use crate::args::Args;
pub use crate::deployment::state_change_layer::StateChangeLayer;
use crate::deployment::{gateway_client::GatewayClient, DeploymentManager};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn start(
    persistence: Persistence,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    log_recorder: impl LogRecorder,
    log_fetcher: LoggerClient<
        shuttle_common::claims::ClaimService<
            shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
        >,
    >,
    builder_client: Option<
        BuilderClient<
            shuttle_common::claims::ClaimService<
                shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
            >,
        >,
    >,
    posthog_client: async_posthog::Client,
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
        .queue_client(GatewayClient::new(args.gateway_uri))
        .log_fetcher(log_fetcher)
        .posthog_client(posthog_client)
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
        args.proxy_fqdn,
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

pub async fn start_proxy(
    proxy_address: SocketAddr,
    fqdn: FQDN,
    address_getter: impl AddressGetter,
) {
    let make_service = make_service_fn(move |socket: &AddrStream| {
        let remote_address = socket.remote_addr();
        let address_getter = address_getter.clone();
        let fqdn = fqdn.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                proxy::handle(remote_address, fqdn.clone(), req, address_getter.clone())
            }))
        }
    });

    let server = hyper::Server::bind(&proxy_address).serve(make_service);

    info!("Starting proxy server on: {}", proxy_address);

    if let Err(e) = server.await {
        error!(error = %e, "proxy died, killing process...");
        std::process::exit(1);
    }
}
