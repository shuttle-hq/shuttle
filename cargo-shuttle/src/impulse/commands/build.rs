use anyhow::{Context, Result};
use nixpacks::nixpacks::{
    builder::docker::DockerBuilderOptions,
    plan::{generator::GeneratePlanOptions, BuildPlan},
};

use crate::impulse::{args::BuildArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn build(&self, build_args: BuildArgs) -> Result<ImpulseCommandOutput> {
        let cwd = &self.global_args.working_directory;
        let dirname = cwd
            .file_name()
            .context("getting name of working directory")?
            .to_string_lossy()
            .into_owned();

        let image_name = dirname;
        nixpacks::create_docker_image(
            build_args.path.as_str(), // TODO: change to global_args.working_directory relative to cwd
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

        Ok(ImpulseCommandOutput::BuiltImage(image_name))
    }
}
