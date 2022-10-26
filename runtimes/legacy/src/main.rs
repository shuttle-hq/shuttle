use std::path::PathBuf;

use clap::Parser;
use shuttle_legacy::{args::Args, Legacy};
use tracing::trace;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    trace!(args = ?args, "parsed args");

    let so_path = PathBuf::from(args.file_path.as_str());
    let mut legacy = Legacy::new();
    legacy.load(so_path).await.unwrap();
    legacy.start().await.unwrap();
}
