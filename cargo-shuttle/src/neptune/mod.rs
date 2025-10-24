pub mod args;

use anyhow::Result;
use nixpacks::nixpacks::{
    builder::docker::DockerBuilderOptions,
    plan::{generator::GeneratePlanOptions, BuildPlan},
};

use crate::{
    args::OutputMode,
    neptune::args::{NeptuneArgs, NeptuneCommand},
};

pub enum NeptuneCommandOutput {
    BuiltImage(String),
    None,
}

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

    pub async fn run(mut self, args: NeptuneArgs) -> Result<NeptuneCommandOutput> {
        self.output_mode = args.output_mode;

        match args.cmd {
            NeptuneCommand::Build(build_args) => {
                eprintln!("Neptune build command");

                let image_name = "test-nixpacks".to_owned();
                nixpacks::create_docker_image(
                    build_args.path.as_str(),
                    Vec::new(),
                    &GeneratePlanOptions {
                        plan: Some(BuildPlan::default()),
                        config_file: None,
                    },
                    &DockerBuilderOptions {
                        name: Some(image_name.clone()),
                        ..Default::default()
                    },
                )
                .await?;

                Ok(NeptuneCommandOutput::BuiltImage(image_name))
            }
        }
    }
}
