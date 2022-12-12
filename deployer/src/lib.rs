use std::{convert::Infallible, net::SocketAddr};

pub use args::Args;
pub use deployment::{
    deploy_layer::DeployLayer, provisioner_factory::AbstractProvisionerFactory,
    runtime_logger::RuntimeLoggerFactory,
};
use deployment::{provisioner_factory, runtime_logger, Built, DeploymentManager};
use fqdn::FQDN;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
};
pub use persistence::Persistence;
use proxy::AddressGetter;
use tracing::{error, info};

mod args;
mod deployment;
mod error;
mod handlers;
mod persistence;
mod proxy;

pub async fn start(
    abstract_factory: impl provisioner_factory::AbstractFactory,
    runtime_logger_factory: impl runtime_logger::Factory,
    persistence: Persistence,
    args: Args,
) {
    let deployment_manager = DeploymentManager::new(
        abstract_factory,
        runtime_logger_factory,
        persistence.clone(),
        persistence.clone(),
        persistence.clone(),
        args.artifacts_path,
    );

    for existing_deployment in persistence.get_all_runnable_deployments().await.unwrap() {
        let built = Built {
            id: existing_deployment.id,
            service_name: existing_deployment.service_name,
            service_id: existing_deployment.service_id,
            tracing_context: Default::default(),
        };
        deployment_manager.run_push(built).await;
    }

    let router = handlers::make_router(
        persistence,
        deployment_manager,
        args.proxy_fqdn,
        args.admin_secret,
        args.project,
    );
    let make_service = router.into_make_service();

    info!("Binding to and listening at address: {}", args.api_address);

    axum::Server::bind(&args.api_address)
        .serve(make_service)
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
