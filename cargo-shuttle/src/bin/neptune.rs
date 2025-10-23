use anyhow::Result;
use cargo_shuttle::neptune::Neptune;
use cargo_shuttle::{neptune::args::NeptuneArgs, setup_tracing};
use clap::{CommandFactory, FromArgMatches};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = NeptuneArgs::command().get_matches();
    let args =
        NeptuneArgs::from_arg_matches(&matches).expect("args to already be parsed successfully");

    setup_tracing(args.debug);

    Neptune::new()?.run(args).await.map(|_| ())
}
