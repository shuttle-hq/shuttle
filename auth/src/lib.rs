mod args;
mod router;

pub use args::Args;
use tracing::info;

pub async fn start(args: Args) {
    let router = router::new();

    info!(address=%args.address, "Binding to and listening at address");

    axum::Server::bind(&args.address)
        .serve(router.into_make_service())
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", args.address));
}
