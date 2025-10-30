use std::env;

use anyhow::{Context, Result};
use nixpacks::nixpacks::{
    builder::docker::DockerBuilderOptions,
    plan::{generator::GeneratePlanOptions, BuildPlan},
};

use crate::impulse::{args::BuildArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn build(&self, build_args: BuildArgs) -> Result<ImpulseCommandOutput> {
        let wd = &self.global_args.working_directory;

        let image_name = if let Some(tag) = build_args.tag {
            tag
        } else {
            self.global_args
                .workdir_name()
                .context("getting name of working directory")?
        };

        // TODO: support detecting an existing Dockerfile (and --dockerfile ...) and building it here, instead of nixpacks.

        // nixpacks takes in a relative path str which is then canonicalized... so calculate the relative path of wd from cwd
        let rel_path = wd
            .strip_prefix(&env::current_dir().unwrap())
            .unwrap()
            .to_str()
            .unwrap();

        nixpacks::create_docker_image(
            rel_path,
            build_args.env.iter().map(|e| e.as_str()).collect(),
            &GeneratePlanOptions {
                plan: Some(BuildPlan::default()),
                config_file: None,
            },
            &DockerBuilderOptions {
                name: Some(image_name.clone()),
                print_dockerfile: build_args.print_dockerfile,
                ..Default::default()
            },
        )
        .await?;

        Ok(ImpulseCommandOutput::BuiltImage(image_name))
    }
}
