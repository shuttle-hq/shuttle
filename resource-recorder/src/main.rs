use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_resource_recorder::args::Args;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), "resource-recorder");

    trace!(args = ?args, "parsed args");
}
