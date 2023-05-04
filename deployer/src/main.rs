use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::args::Args;
use shuttle_deployer::handlers::make_router;
use tracing::trace;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), "deployer");
    let router = make_router(args.auth_uri).await;
    axum::Server::bind(&args.api_address)
        .serve(router.into_make_service())
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", args.api_address));
}
