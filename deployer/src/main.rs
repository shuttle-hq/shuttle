use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_deployer::args::Args;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), "deployer");
}
