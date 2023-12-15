use std::{process::exit, time::Duration};

use async_posthog::ClientOptions;
use clap::Parser;
use shuttle_common::{
    backends::tracing::setup_tracing,
    claims::{ClaimLayer, InjectPropagationLayer},
    log::{Backend, DeploymentLogLayer},
};
use shuttle_deployer::{start, start_proxy, Args, Persistence, RuntimeManager, StateChangeLayer};
use shuttle_proto::{
    builder::builder_client::BuilderClient,
    logger::{logger_client::LoggerClient, Batcher},
};
use tokio::select;
use tower::ServiceBuilder;
use tracing::{error, trace};
use tracing_subscriber::prelude::*;
use ulid::Ulid;

// The `multi_thread` is needed to prevent a deadlock in shuttle_service::loader::build_crate() which spawns two threads
// Without this, both threads just don't start up
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    let (persistence, _) = Persistence::new(
        &args.state,
        &args.resource_recorder,
        &args.provisioner_address,
        Ulid::from_string(args.project_id.as_str())
            .expect("to get a valid ULID for project_id arg"),
    )
    .await;

    let channel = ServiceBuilder::new()
        .layer(ClaimLayer)
        .layer(InjectPropagationLayer)
        .service(
            args.logger_uri
                .connect()
                .await
                .expect("failed to connect to logger"),
        );
    let logger_client = LoggerClient::new(channel);
    let logger_batcher = Batcher::wrap(logger_client.clone());

    let builder_client = match args.builder_uri.connect().await {
        Ok(channel) => Some(BuilderClient::new(
            ServiceBuilder::new()
                .layer(ClaimLayer)
                .layer(InjectPropagationLayer)
                .service(channel),
        )),
        Err(err) => {
            error!("Couldn't connect to the shuttle-builder: {err}");
            None
        }
    };

    let ph_client_options = ClientOptions::new(
        args.posthog_key.to_string(),
        "https://eu.posthog.com".to_string(),
        Duration::from_millis(800),
    );

    let posthog_client = async_posthog::client(ph_client_options);

    setup_tracing(
        tracing_subscriber::registry()
            .with(StateChangeLayer {
                log_recorder: logger_batcher.clone(),
                state_recorder: persistence.clone(),
            })
            // TODO: Make all relevant backends set this up in this way
            .with(DeploymentLogLayer {
                log_recorder: logger_batcher.clone(),
                internal_service: Backend::Deployer,
            }),
        Backend::Deployer,
        None,
    );

    let runtime_manager = RuntimeManager::new(
        args.provisioner_address.to_string(),
        logger_batcher.clone(),
        Some(args.auth_uri.to_string()),
    );

    select! {
        _ = start_proxy(args.proxy_address, args.proxy_fqdn.clone(), persistence.clone()) => {
            error!("Proxy stopped.")
        },
        _ = start(persistence, runtime_manager, logger_batcher, logger_client, builder_client, posthog_client, args) => {
            error!("Deployment service stopped.")
        },
    }

    exit(1);
}
