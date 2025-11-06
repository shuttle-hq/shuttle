use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
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

    pub async fn generate_agents(&self, dir: impl AsRef<Path>) -> Result<ImpulseCommandOutput> {
        let dir = dir.as_ref().canonicalize()?;
        if !dir.exists() && !dir.is_dir() {
            fs::create_dir_all(&dir)?;
        }
        let file = dir.join("AGENTS.md");

        let re = regex::Regex::new(r"<!-- impulse: agents.md version (.+) -->").unwrap();
        // let re =
        //     regex::Regex::new(r"<!-- impulse: agents.md version (.+) -->.+<!-- impulse end -->")
        //         .unwrap();
        let agents = self.client.get_agents_md().await?;
        let agents_version = re
            .captures(&agents)
            .context("detecting remote AGENTS.md version")?
            .get(1)
            .unwrap()
            .as_str();
        tracing::debug!("got agents.md file with version {}", agents_version);

        tracing::debug!("checking {} for existing impulse rules", file.display());
        if file.exists() && file.is_file() {
            let mut content = fs::read_to_string(&file).context("reading existing AGENTS.md")?;
            if let Some(cap) = re.captures(&content) {
                let mat = cap.get(0).unwrap();
                let version = cap.get(1).unwrap().as_str();
                if version < agents_version {
                    tracing::debug!("updating agents.md impulse section");
                    let before = &content[0..mat.start()];
                    let after = &content[mat.end()..];
                    content = format!("{before}{agents}{after}");
                    fs::write(&file, content.as_bytes()).context("writing AGENTS.md")?;
                } else {
                    tracing::info!("AGENTS.md instructions are up to date");
                }
            } else {
                // append
                content.push('\n');
                content.push('\n');
                content.push_str(&agents);
                fs::write(&file, content.as_bytes()).context("writing AGENTS.md")?;
            }
        } else {
            tracing::debug!("not found, creating");
            let mut f = fs::File::create(&file).context("creating AGENTS.md")?;
            f.write_all(agents.as_bytes())
                .context("writing AGENTS.md")?;
        }

        Ok(ImpulseCommandOutput::None)
    }

    pub async fn generate_spec(&self, dir: impl AsRef<Path>) -> Result<ImpulseCommandOutput> {
        let bytes: Vec<u8> = self.create_build_context(&dir, super::build::ArchiveType::Zip)?;

        let spec_bytes = self.client.generate_impulse_spec(bytes).await?;

        tokio::fs::write(dir.as_ref().join("shuttle.json"), spec_bytes).await?;

        // Output success message based on mode
        if self.global_args.output_mode == crate::OutputMode::Json {
            eprintln!(indoc::indoc! {r#"
                {{
                    "ok": true,
                    "summary": "Generated shuttle.json configuration",
                    "messages": [
                        "Created shuttle.json at the project root.",
                    ],
                    "next_action": "validate_then_deploy",
                    "requires_confirmation": false,
                    "next_action_tool": "impulse deploy",
                    "next_action_params": {{}},
                    "next_action_non_tool": "Review the generated shuttle.json configuration and run 'impulse status' to check deployment readiness."
                }}"#
            });
        } else if self.global_args.verbose {
            eprintln!(indoc::indoc! {r#"
                SUCCESS: Generated shuttle.json configuration

                The shuttle.json project manifest has been successfully created at the project root.
                This file contains your project configuration including:
                - Project name and metadata
                - Resource definitions (databases, secrets, etc.)
                - Runtime and deployment settings

                Next steps:
                1. Review the generated shuttle.json configuration
                2. Run 'impulse deploy' to build and deploy your application
                3. Run 'impulse status' to check your project deployment

                The configuration is based on your project's source code analysis and
                follows Shuttle's best practices for resource provisioning.
                "#
            });
        } else {
            eprintln!("Generated shuttle.json - review the configuration and run 'impulse deploy' to deploy");
        }

        Ok(ImpulseCommandOutput::None)
    }
}
