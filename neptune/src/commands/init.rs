use std::{ffi::OsString, fs};

use anyhow::Result;
use cargo_shuttle::{args::parse_and_create_path, init::generate_project};
use dialoguer::{theme::ColorfulTheme, Confirm, Input};

use crate::{args::InitArgs, Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn init(&self, args: InitArgs) -> Result<NeptuneCommandOutput> {
        // TODO: offer to log in if not done yet?

        let theme = ColorfulTheme::default();
        let git_template = args.git_template()?;
        let no_git = args.no_git;
        let needs_path = !args.path_provided_arg;
        let project_name: String = Input::with_theme(&theme)
            .with_prompt("Project name")
            .interact()?;
        // TODO: validate project name

        // Confirm the project directory
        let path = if needs_path {
            let path = args.path.join(&project_name);

            loop {
                eprintln!("Where should we create this project?");

                let directory_str: String = Input::with_theme(&theme)
                    .with_prompt("Directory")
                    .default(format!("{}", path.display()))
                    .interact()?;
                eprintln!();

                let path = parse_and_create_path(OsString::from(directory_str))?;

                if fs::read_dir(&path)
                    .expect("init dir to exist and list entries")
                    .count()
                    > 0
                    && !Confirm::with_theme(&theme)
                        .with_prompt("Target directory is not empty. Are you sure?")
                        .default(true)
                        .interact()?
                {
                    eprintln!();
                    continue;
                }

                break path;
            }
        } else {
            args.path.clone()
        };

        // Resolve template
        // TODO: provide a list of starter templates to choose from if --from is not used
        let git_template =
            git_template.ok_or(anyhow::anyhow!("No template provided. Use --from"))?;

        generate_project(path.clone(), &project_name, &git_template, no_git)?;
        eprintln!();

        if Confirm::with_theme(&theme)
            .with_prompt("Generate AGENTS.md with AI instructions?")
            .default(true)
            .interact()?
        {
            self.generate_agents(&path).await?;
        }

        if std::env::current_dir().is_ok_and(|d| d != path) {
            eprintln!("You can `cd` to the directory, then:");
        }
        eprintln!("Run `neptune deploy` to deploy it.");

        Ok(NeptuneCommandOutput::None)
    }
}
