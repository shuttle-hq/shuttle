use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use anyhow::{Context, Result};
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
        let re =
            regex::Regex::new(r"<!-- impulse: agents.md version (.+) -->.+<!-- impulse end -->")
                .unwrap();
        let agents = self.client.get_agents_md().await?;
        let agents_version = re
            .captures(&agents)
            .context("detecting remote AGENTS.md version")?
            .get(1)
            .unwrap()
            .as_str();
        tracing::debug!("got agents.md file with version {}", agents_version);
        let p = self.global_args.working_directory.join("AGENTS.md");
        tracing::debug!("checking {} for existing impulse rules", p.display());
        if p.exists() && p.is_file() {
            let mut content = fs::read_to_string(&p).context("reading existing AGENTS.md")?;
            if let Some(cap) = re.captures(&content) {
                let mat = cap.get(0).unwrap();
                let version = cap.get(1).unwrap().as_str();
                if version < agents_version {
                    tracing::debug!("updating agents.md impulse section");
                    let before = &content[0..mat.start()];
                    let after = &content[mat.end()..];
                    content = format!("{before}{agents}{after}");
                    fs::File::open(&p)
                        .context("updating AGENTS.md")?
                        .write_all(content.as_bytes())
                        .context("writing AGENTS.md")?;
                } else {
                    tracing::info!("AGENTS.md instructions are up to date");
                }
            } else {
                // append
                content.push('\n');
                content.push('\n');
                content.push_str(&agents);
                fs::File::open(&p)
                    .context("updating AGENTS.md")?
                    .write_all(content.as_bytes())
                    .context("writing AGENTS.md")?;
            }
        } else {
            tracing::debug!("not found, creating");
            let mut f = fs::File::create(&p).context("creating AGENTS.md")?;
            f.write_all(agents.as_bytes())
                .context("writing AGENTS.md")?;
        }

        Ok(ImpulseCommandOutput::None)
    }

    pub async fn generate_spec(&self) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }
}
