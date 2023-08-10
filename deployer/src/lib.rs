use std::{convert::Infallible, net::SocketAddr, sync::Arc};

pub use args::Args;
pub use deployment::deploy_layer::DeployLayer;
use deployment::DeploymentManager;
use fqdn::FQDN;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
};
pub use persistence::Persistence;
use proxy::AddressGetter;
pub use runtime_manager::RuntimeManager;
use tokio::sync::Mutex;
use tracing::{error, info};
use ulid::Ulid;

use crate::deployment::gateway_client::GatewayClient;

mod args;
mod deployment;
mod error;
pub mod handlers;
mod persistence;
mod proxy;
mod runtime_manager;

pub async fn start(
    persistence: Persistence,
    runtime_manager: Arc<Mutex<RuntimeManager>>,
    args: Args,
) {
    // when _set is dropped once axum exits, the deployment tasks will be aborted.
    let deployment_manager = DeploymentManager::builder()
        .build_log_recorder(persistence.clone())
        .secret_recorder(persistence.clone())
        .active_deployment_getter(persistence.clone())
        .artifacts_path(args.artifacts_path)
        .runtime(runtime_manager)
        .deployment_updater(persistence.clone())
        .secret_getter(persistence.clone())
        .resource_manager(persistence.clone())
        .queue_client(GatewayClient::new(args.gateway_uri))
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
