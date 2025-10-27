use anyhow::Result;
use cargo_shuttle::impulse::Impulse;
use cargo_shuttle::{impulse::args::ImpulseArgs, setup_tracing};
use clap::{CommandFactory, FromArgMatches};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = ImpulseArgs::command().get_matches();
    let args =
        ImpulseArgs::from_arg_matches(&matches).expect("args to already be parsed successfully");

    setup_tracing(args.globals.debug);

    Impulse::new(args.globals)?.run(args.cmd).await.map(|_| ())
}
