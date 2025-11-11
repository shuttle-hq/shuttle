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
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use crossterm::style::Stylize;
use indicatif::{ProgressBar, ProgressStyle};
use pretty_assertions::StrComparison;
use shuttle_api_client::neptune_types::GenerateResponse;

use crate::{args::NeptuneArgs, Neptune, NeptuneCommandOutput};

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

        // User-facing progress output (avoid in JSON mode)
        if self.global_args.output_mode != OutputMode::Json {
            eprintln!("Generating or updating {}", file.display());
            eprintln!("Fetching latest Neptune agent instructions...");
        }

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
            if self.global_args.output_mode != OutputMode::Json {
                eprintln!("Found existing {}", file.display());
            }
            let mut content = fs::read_to_string(&file).context("reading existing AGENTS.md")?;
            if let Some(cap) = re.captures(&content) {
                let mat = cap.get(0).unwrap();
                let version = cap.get(1).unwrap().as_str();
                if version < agents_version {
                    tracing::debug!("updating agents.md neptune section");
                    if self.global_args.output_mode != OutputMode::Json {
                        eprintln!(
                            "Updating Neptune instructions in AGENTS.md ({} -> {})",
                            version, agents_version
                        );
                    }
                    let before = &content[0..mat.start()];
                    let after = &content[mat.end()..];
                    content = format!("{before}{agents}{after}");
                    fs::write(&file, content.as_bytes()).context("writing AGENTS.md")?;
                    changed = true;
                } else {
                    tracing::info!("AGENTS.md instructions are up to date");
                    if self.global_args.output_mode != OutputMode::Json {
                        eprintln!("AGENTS.md instructions are up to date");
                    }
                }
            } else {
                // append
                if self.global_args.output_mode != OutputMode::Json {
                    eprintln!("Appending Neptune instructions to AGENTS.md");
                }
                content.push('\n');
                content.push('\n');
                content.push_str(&agents);
                fs::write(&file, content.as_bytes()).context("writing AGENTS.md")?;
                changed = true;
            }
        } else {
            tracing::debug!("not found, creating");
            if self.global_args.output_mode != OutputMode::Json {
                eprintln!("Creating {}", file.display());
            }
            let mut f = fs::File::create(&file).context("creating AGENTS.md")?;
            f.write_all(agents.as_bytes())
                .context("writing AGENTS.md")?;
            changed = true;
        }

        if changed && self.global_args.output_mode != OutputMode::Json {
            eprintln!("Done");
        }

        Ok(NeptuneCommandOutput::None)
    }

    pub async fn generate_spec(
        &self,
        dir: impl AsRef<Path> + Sync,
    ) -> Result<NeptuneCommandOutput> {
        let bytes: Vec<u8> = self.create_build_context(
            &dir,
            super::build::ArchiveType::Zip,
            None::<Vec<&Path>>,
            true,
        )?;

        // Spinner only for non-JSON output
        let spinner = if self.global_args.output_mode != OutputMode::Json {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("{spinner:.green} {msg}")
                    .unwrap()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
            );
            pb.set_message("Analyzing project and generating configuration...");
            pb.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(pb)
        } else {
            None
        };

        let gen_res: GenerateResponse = self.client.generate(bytes, "hickyblue").await?;

        if let Some(pb) = spinner.as_ref() {
            pb.finish_and_clear();
        }

        // Write neptune.json with change detection and small preview if updated
        let spec_path = dir.as_ref().join("neptune.json");
        let new_spec_pretty = serde_json::to_string_pretty(&gen_res.platform_spec)?;
        if spec_path.exists() && spec_path.is_file() {
            if let Ok(existing) = fs::read_to_string(&spec_path) {
                // Normalize existing by parsing and re-serializing to pretty JSON to avoid whitespace-only diffs
                let normalized_existing = serde_json::from_str::<serde_json::Value>(&existing)
                    .ok()
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
                    .unwrap_or(existing.clone());
                let changed = normalized_existing != new_spec_pretty;
                if !changed && self.global_args.output_mode != OutputMode::Json {
                    eprintln!("neptune.json is up to date");
                }
                if changed && self.global_args.output_mode != OutputMode::Json {
                    eprintln!("Updating neptune.json... (changes shown below)");
                    eprintln!();
                    eprintln!("--- neptune.json ---");
                    // Use pretty_assertions to render a formatted, colored diff of the full JSON
                    let diff = format!(
                        "{}",
                        StrComparison::new(&normalized_existing, &new_spec_pretty)
                    );
                    // Show only the first 60 lines as a preview
                    let max_lines = 60usize;
                    for (i, line) in diff.lines().enumerate() {
                        if i >= max_lines {
                            eprintln!("... (truncated preview)");
                            break;
                        }
                        eprintln!("{}", line);
                    }
                    eprintln!();
                    eprintln!(
                        "(Tip: run `git --no-pager diff -- neptune.json` to see full changes)"
                    );
                }
            }
        } else if self.global_args.output_mode != OutputMode::Json {
            eprintln!("Creating neptune.json");
        }
        tokio::fs::write(&spec_path, new_spec_pretty.as_bytes()).await?;

        // Output success message based on mode
        if self.global_args.output_mode == OutputMode::Json {
            eprintln!(indoc::indoc! {r#"
                {{
                    "ok": true,
                    "summary": "Generated neptune.json configuration",
                    "messages": [
                        "Created neptune.json at the project root.",
                    ],
                    "next_action": "deploy",
                    "requires_confirmation": false,
                    "next_action_tool": "neptune deploy",
                    "next_action_params": {{}},
                    "next_action_non_tool": "Review the generated neptune.json configuration and run 'neptune status' to check deployment readiness."
                }}"#
            });
        } else if self.global_args.verbose {
            eprintln!(indoc::indoc! {r#"
                SUCCESS: Generated neptune.json configuration

                The neptune.json project manifest has been successfully created at the project root.
                This file contains your project configuration including:
                - Project name and metadata
                - Resource definitions (databases, secrets, etc.)
                - Runtime and deployment settings

                Next steps:
                1. Review the generated neptune.json configuration
                2. Run 'neptune deploy' to build and deploy your application
                3. Run 'neptune status' to check your project deployment

                The configuration is based on your project's source code analysis and
                follows Shuttle's best practices for resource provisioning.
                "#
            });
        }

        // Handle compatibility report: print if incompatible
        if !gen_res.compatibility_report.compatible {
            if self.global_args.output_mode == OutputMode::Json {
                let report_json = serde_json::to_string_pretty(&gen_res.compatibility_report)?;
                eprintln!("{}", report_json);
            } else {
                eprintln!();
                eprintln!("{}", "Possible compatibility issues detected:".yellow());
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .set_header(vec![
                        Cell::new("Category").add_attribute(Attribute::Bold),
                        Cell::new("Message").add_attribute(Attribute::Bold),
                        Cell::new("Path").add_attribute(Attribute::Bold),
                        Cell::new("Suggestion").add_attribute(Attribute::Bold),
                    ]);
                for err in gen_res.compatibility_report.errors.iter() {
                    let category = match err.category {
                        shuttle_api_client::neptune_types::ErrorCategory::Architecture => {
                            "Architecture"
                        }
                        shuttle_api_client::neptune_types::ErrorCategory::ResourceSupport => {
                            "Resource Support"
                        }
                        shuttle_api_client::neptune_types::ErrorCategory::WorkloadSupport => {
                            "Workload Support"
                        }
                        shuttle_api_client::neptune_types::ErrorCategory::ConfigurationInvalid => {
                            "Configuration Invalid"
                        }
                        shuttle_api_client::neptune_types::ErrorCategory::Unknown => "Other",
                    };
                    table.add_row(vec![
                        Cell::new(category).fg(Color::White),
                        Cell::new(err.message.as_str()).fg(Color::Red),
                        Cell::new(err.path.as_deref().unwrap_or("-")).fg(Color::White),
                        Cell::new(err.suggestion.as_deref().unwrap_or("-")).fg(Color::White),
                    ]);
                }
                if table.row_count() > 1 {
                    eprintln!("{}", table);
                }
                eprintln!();
            }
        }

        // Save start_command for build usage, .neptune/start_command
        let start_file = dir.as_ref().join(".neptune").join("start_command");
        if let Some(parent) = start_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        tokio::fs::write(&start_file, gen_res.start_command.as_bytes()).await?;

        Ok(NeptuneCommandOutput::None)
    }
}
