use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use clap_mangen::Man;

use crate::impulse::{args::ImpulseArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn generate_completions(
        &self,
        shell: Shell,
        output_file: Option<PathBuf>,
    ) -> Result<ImpulseCommandOutput> {
        let name = "impulse";
        let mut app = ImpulseArgs::command();
        let mut output = Vec::new();

        generate(shell, &mut app, name, &mut output);
        match output_file {
            Some(path) => fs::File::create(path)?.write(&output)?,
            None => io::stdout().write(&output)?,
        };

        Ok(ImpulseCommandOutput::None)
    }

    pub async fn generate_manpage(
        &self,
        output_file: Option<PathBuf>,
    ) -> Result<ImpulseCommandOutput> {
        let app = ImpulseArgs::command();
        let mut output = Vec::new();

        Man::new(app.clone()).render(&mut output)?;

        for subcommand in app.get_subcommands() {
            let primary = Man::new(subcommand.clone());
            primary.render_name_section(&mut output)?;
            primary.render_synopsis_section(&mut output)?;
            primary.render_description_section(&mut output)?;
            primary.render_options_section(&mut output)?;
            // For example, `generate` has sub-commands `shell` and `manpage`
            if subcommand.has_subcommands() {
                primary.render_subcommands_section(&mut output)?;
                for sb in subcommand.get_subcommands() {
                    let secondary = Man::new(sb.clone());
                    secondary.render_name_section(&mut output)?;
                    secondary.render_synopsis_section(&mut output)?;
                    secondary.render_description_section(&mut output)?;
                    secondary.render_options_section(&mut output)?;
                }
            }
        }

        match output_file {
            Some(path) => fs::File::create(path)?.write(&output)?,
            None => io::stdout().write(&output)?,
        };

        Ok(ImpulseCommandOutput::None)
    }

    pub async fn generate_agents(&self) -> Result<ImpulseCommandOutput> {
        let a = self.client.get_agents_md().await?;
        println!("{a}");
        // TODO: write it to the file
        Ok(ImpulseCommandOutput::None)
    }

    pub async fn generate_spec(&self) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }
}
