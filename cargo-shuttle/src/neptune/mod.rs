use anyhow::Result;

use crate::{
    args::OutputMode,
    neptune::args::{NeptuneArgs, NeptuneCommand},
    CommandOutput,
};

pub mod args;

pub struct Neptune {
    // ctx: RequestContext,
    // client: Option<ShuttleApiClient>,
    output_mode: OutputMode,
    // /// Alter behaviour based on which CLI is used
    // bin: Binary,
}

impl Neptune {
    pub fn new(/* bin: Binary */ /* env_override: Option<String> */) -> Result<Self> {
        // let ctx = RequestContext::load_global(env_override.inspect(|e| {
        //     eprintln!(
        //         "{}",
        //         format!("INFO: Using non-default global config file: {e}").yellow(),
        //     )
        // }))?;
        Ok(Self {
            // ctx,
            // client: None,
            output_mode: OutputMode::Normal,
            // bin,
        })
    }

    pub async fn run(mut self, args: NeptuneArgs) -> Result<CommandOutput> {
        self.output_mode = args.output_mode;

        match args.cmd {
            NeptuneCommand::Build(_build_args) => {
                eprintln!("Neptune build command");
                Ok(CommandOutput::None)
            }
        }
    }
}
