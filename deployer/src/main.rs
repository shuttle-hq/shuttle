use clap::Parser;
use shuttle_common::{
    backends::trace::setup_tracing,
    log::{Backend, DeploymentLogLayer},
};
use shuttle_deployer::{start, Args, Persistence, RuntimeManager, StateChangeLayer};
use shuttle_proto::{
    // builder::builder_client::BuilderClient,
    logger::{self, Batcher},
};
use tracing::trace;
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
        args.resource_recorder.clone(),
        args.provisioner_address.clone(),
        Ulid::from_string(args.project_id.as_str())
            .expect("to get a valid ULID for project_id arg"),
    )
    .await;

    let logger_client = logger::get_client(args.logger_uri.clone()).await;
    let logger_batcher = Batcher::wrap(logger_client.clone());

    let builder_client = None;
    // let builder_client = match args.builder_uri.connect().await {
    //     Ok(channel) => Some(BuilderClient::new(
    //         ServiceBuilder::new()
    //             .layer(ClaimLayer)
    //             .layer(InjectPropagationLayer)
    //             .service(channel),
    //     )),
    //     Err(err) => {
    //         error!("Couldn't connect to the shuttle-builder: {err}");
    //         None
    //     }
    // };

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

    start(
        persistence,
        runtime_manager,
        logger_batcher,
        logger_client,
        builder_client,
        args,
    )
    .await
}
