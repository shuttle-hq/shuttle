use anyhow::Result;
use cargo_shuttle::{reload_env_filter, setup_tracing};
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};
use neptune::args::{NeptuneArgs, NeptuneCommand};
use neptune::Neptune;

#[tokio::main]
async fn main() -> Result<()> {
    // set up tracing with debug off. debug flag can't be enabled this early
    let env_filter_handle = setup_tracing(false);
    tracing::info!("Neptune CLI starting");

    tracing::info!("Parsing args");
    let matches = NeptuneArgs::command().get_matches();
    let mut args =
        NeptuneArgs::from_arg_matches(&matches).expect("args to already be parsed successfully");
    // store which of the args with default values that were given on the command line
    // TODO: find a way to not hardcode this list
    for arg in ["debug", "output_mode"] {
        if matches.value_source(arg) == Some(ValueSource::CommandLine) {
            args.globals.arg_provided_fields.push(arg);
        }
    }
    if let NeptuneCommand::Init(ref mut init_args) = args.cmd {
        let provided_path_to_init =
            matches
                .subcommand_matches("init")
                .is_some_and(|init_matches| {
                    init_matches.value_source("path") == Some(ValueSource::CommandLine)
                });
        init_args.path_provided_arg = provided_path_to_init;
    }

    // reload to enable debugging asap if given as arg or env var
    reload_env_filter(&env_filter_handle, args.globals.debug);

    Neptune::new(args.globals, Some(env_filter_handle))?
        .run(args.cmd)
        .await
        .map(|_| ())
}
