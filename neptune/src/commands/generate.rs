use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use cargo_shuttle::args::OutputMode;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use clap_mangen::Man;
use crossterm::style::Stylize;
use serde::Serialize;
use shuttle_api_client::neptune_types::GenerateResponse;

use crate::{args::NeptuneArgs, ui::AiUi, Neptune, NeptuneCommandOutput};

use super::common::{
    generate_platform_spec, make_spinner, preview_spec_changes,
    print_compatibility_report_if_needed, write_start_command, SpecPreviewStatus,
};

#[derive(Serialize)]
struct GenerateJsonOutput {
    ok: bool,
    spec_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
    next_action_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<shuttle_api_client::neptune_types::CompatibilityReport>,
}

impl Neptune {
    pub async fn generate_completions(
        &self,
        shell: Shell,
        output_file: Option<PathBuf>,
    ) -> Result<NeptuneCommandOutput> {
        let name = "neptune";
        let mut app = NeptuneArgs::command();
        let mut output = Vec::new();

        generate(shell, &mut app, name, &mut output);
        match output_file {
            Some(path) => fs::File::create(path)?.write(&output)?,
            None => io::stdout().write(&output)?,
        };

        Ok(NeptuneCommandOutput::None)
    }

    pub async fn generate_manpage(
        &self,
        output_file: Option<PathBuf>,
    ) -> Result<NeptuneCommandOutput> {
        let app = NeptuneArgs::command();
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

        Ok(NeptuneCommandOutput::None)
    }

    pub async fn generate_agents(&self, dir: impl AsRef<Path>) -> Result<NeptuneCommandOutput> {
        let dir = dir.as_ref();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        let dir = dir.canonicalize()?;
        let file = dir.join("AGENTS.md");
        let mut changed = false;

        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("AGENTS.md");
        ui.step("", format!("Generating/updating {}", file.display()));
        ui.step("", "Fetching latest Neptune agent instructions...");

        let re = regex::Regex::new(
            r"(?s)<!-- neptune: agents\.md version ([^>]+) -->.*?<!-- neptune end -->",
        )
        .unwrap();
        // let re =
        //     regex::Regex::new(r"<!-- neptune: agents.md version (.+) -->.+<!-- neptune end -->")
        //         .unwrap();
        let agents = self.client.get_agents_md().await?;
        let agents_version = re
            .captures(&agents)
            .context("detecting remote AGENTS.md version")?
            .get(1)
            .unwrap()
            .as_str();
        tracing::debug!("got agents.md file with version {}", agents_version);

        tracing::debug!("checking {} for existing neptune rules", file.display());
        if file.exists() && file.is_file() {
            ui.step("", format!("Found existing {}", file.display()));
            let mut content = fs::read_to_string(&file).context("reading existing AGENTS.md")?;
            if let Some(cap) = re.captures(&content) {
                let mat = cap.get(0).unwrap();
                let version = cap.get(1).unwrap().as_str();
                if version < agents_version {
                    tracing::debug!("updating agents.md neptune section");
                    ui.step(
                        "",
                        format!(
                            "Updating Neptune instructions version (Current: {}, New: {})",
                            version.to_string().yellow(),
                            agents_version.to_string().green()
                        ),
                    );
                    let before = &content[0..mat.start()];
                    let after = &content[mat.end()..];
                    content = format!("{before}{agents}{after}");
                    fs::write(&file, content.as_bytes()).context("writing AGENTS.md")?;
                    changed = true;
                } else {
                    tracing::info!("AGENTS.md instructions are up to date");
                    ui.success("✅ AGENTS.md up to date");
                }
            } else {
                // append
                ui.step("", "Appending Neptune instructions to AGENTS.md");
                content.push('\n');
                content.push('\n');
                content.push_str(&agents);
                fs::write(&file, content.as_bytes()).context("writing AGENTS.md")?;
                changed = true;
            }
        } else {
            tracing::debug!("not found, creating");
            ui.step("", format!("Creating {}", file.display()));
            let mut f = fs::File::create(&file).context("creating AGENTS.md")?;
            f.write_all(agents.as_bytes())
                .context("writing AGENTS.md")?;
            changed = true;
        }

        if changed && self.global_args.output_mode != OutputMode::Json {
            ui.success("✅ AGENTS.md updated");
        }

        Ok(NeptuneCommandOutput::None)
    }

    pub async fn generate_spec(
        &self,
        dir: impl AsRef<Path> + Sync,
    ) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("neptune.json");
        let mut printed_up_to_date = false;

        let project_name = self.resolve_project_name().await?;

        // Prepare consolidated JSON output if in JSON mode
        let mut json_out = if self.global_args.output_mode == OutputMode::Json {
            Some(GenerateJsonOutput {
                ok: true,
                spec_path: String::new(),
                messages: None,
                next_action_command: String::new(),
                compatibility: None,
            })
        } else {
            None
        };

        let spinner = make_spinner(
            &self.global_args.output_mode,
            "Analyzing project and generating configuration...",
        );

        let gen_res: GenerateResponse =
            match generate_platform_spec(self, dir.as_ref(), &project_name).await {
                Ok(v) => v,
                Err(e) => {
                    if let Some(ref mut out) = json_out {
                        out.ok = false;
                        out.messages = Some(vec![
                            "Failed to analyze project and generate configuration".to_string(),
                            format!("Project: {}", project_name),
                            format!("Error: {}", e),
                        ]);
                        out.next_action_command = "neptune generate spec".to_string();
                        println!("{}", serde_json::to_string_pretty(&out)?);
                        return Ok(NeptuneCommandOutput::None);
                    } else {
                        return Err(e);
                    }
                }
            };

        if let Some(pb) = spinner.as_ref() {
            pb.finish_and_clear();
        }

        // Write neptune.json with change detection and small preview if updated
        let spec_path = dir.as_ref().join("neptune.json");
        let new_spec_pretty = serde_json::to_string_pretty(&gen_res.platform_spec)?;
        match preview_spec_changes(
            &spec_path,
            &new_spec_pretty,
            &ui,
            &self.global_args.output_mode,
        )? {
            SpecPreviewStatus::UpToDate => printed_up_to_date = true,
            _ => {}
        };

        tokio::fs::write(&spec_path, new_spec_pretty.as_bytes()).await?;

        // Output success message or prepare JSON summary
        if let Some(ref mut out) = json_out {
            out.spec_path = spec_path.display().to_string();
            if !printed_up_to_date {
                out.messages = Some(vec![format!(
                    "Created or updated neptune.json at {}",
                    spec_path.display()
                ), format!("Review the generated neptune.json configuration to ensure it is correct and represents your project's requirements.")]);
                out.next_action_command = "neptune deploy".to_string();
            }
        } else {
            if !printed_up_to_date {
                eprintln!();
                ui.success("✅ Generated neptune.json");
            }
        }

        // Handle compatibility report: print if incompatible
        if !gen_res.compatibility_report.compatible {
            if let Some(ref mut out) = json_out {
                out.compatibility = Some(gen_res.compatibility_report.clone());
            } else {
                print_compatibility_report_if_needed(
                    &ui,
                    &gen_res.compatibility_report,
                    &self.global_args.output_mode,
                );
            }
        }

        // Save start_command for build usage, .neptune/start_command
        write_start_command(dir.as_ref(), &gen_res.start_command).await?;

        // Print consolidated JSON (single object) at the end if in JSON mode
        if let Some(out) = json_out {
            println!("{}", serde_json::to_string_pretty(&out)?);
        }

        Ok(NeptuneCommandOutput::None)
    }
}
