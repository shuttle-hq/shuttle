use anyhow::Result;
use cargo_shuttle::impulse::Impulse;
use cargo_shuttle::reload_env_filter;
use cargo_shuttle::{impulse::args::ImpulseArgs, setup_tracing};
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};

#[tokio::main]
async fn main() -> Result<()> {
    // set up tracing with debug off. debug flag can't be enabled this early
    let env_filter_handle = setup_tracing(false);
    tracing::info!("Impulse CLI starting");

    tracing::info!("Parsing args");
    let matches = ImpulseArgs::command().get_matches();
    let mut args =
        ImpulseArgs::from_arg_matches(&matches).expect("args to already be parsed successfully");
    // store which of the args with default values that were given on the command line
    // TODO: find a way to not hardcode this list
    for arg in ["debug", "output_mode"] {
        if matches.value_source(arg) == Some(ValueSource::CommandLine) {
            args.globals.arg_provided_fields.push(arg);
        }
    }

    // reload to enable debugging asap if given as arg or env var
    reload_env_filter(&env_filter_handle, args.globals.debug);

    Impulse::new(args.globals, Some(env_filter_handle))?
        .run(args.cmd)
        .await
        .map(|_| ())
}
