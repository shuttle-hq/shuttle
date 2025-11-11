use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use crossterm::style::Stylize;
use dialoguer::Confirm;
use impulse_common::types::{ProjectState, ResourcesState, WorkloadState};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::time::Duration;
use tokio::time::sleep;

use crate::{args::DeployArgs, Neptune, NeptuneCommandOutput};

#[derive(Serialize)]
struct BuildSummary {
    image: Option<String>,
}

#[derive(Serialize)]
struct DeploymentSummary {
    project: String,
    deployment_id: String,
    summary: String,
    messages: Vec<String>,
    next_action: String,
    requires_confirmation: bool,
}

#[derive(Serialize)]
struct ErrorPayload {
    code: String,
    message: String,
    details: serde_json::Value,
    next_action: Option<String>,
    requires_confirmation: bool,
}

#[derive(Serialize)]
struct DeployJsonOutput {
    ok: bool,
    project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<shuttle_api_client::neptune_types::CompatibilityReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<BuildSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deployment: Option<DeploymentSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_condition: Option<impulse_common::types::AggregateProjectCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorPayload>,
}

impl Neptune {
    pub async fn deploy(&self, deploy_args: DeployArgs) -> Result<NeptuneCommandOutput> {
        // Determine project name for /v1/generate from working directory
        let project_name = self
            .global_args
            .workdir_name()
            .unwrap_or_else(|| String::from("project"));

        // In JSON mode, we collect a single consolidated result to print at the end
        let mut json_out = if self.global_args.output_mode == OutputMode::Json {
            Some(DeployJsonOutput {
                ok: false,
                project: project_name.clone(),
                compatibility: None,
                build: None,
                deployment: None,
                final_condition: None,
                error: None,
            })
        } else {
            None
        };

        // Run spec generation preview (same flow as `generate spec`) and ask for confirmation
        let gen_spinner = if self.global_args.output_mode != OutputMode::Json {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("{spinner:.green} {msg}")
                    .unwrap()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
            );
            pb.set_message("Analyzing project and generating configuration preview...");
            pb.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(pb)
        } else {
            None
        };
        let bytes: Vec<u8> = self.create_build_context(
            &self.global_args.working_directory,
            crate::commands::build::ArchiveType::Zip,
            None::<Vec<std::path::PathBuf>>,
            true,
        )?;
        let gen_res = self.client.generate(bytes, &project_name).await?;
        if let Some(pb) = gen_spinner.as_ref() {
            pb.finish_and_clear();
        }

        // Write neptune.json with change detection and small preview if updated
        let dir = &self.global_args.working_directory;
        let spec_path = dir.join("neptune.json");
        let new_spec_pretty = serde_json::to_string_pretty(&gen_res.platform_spec)?;
        if spec_path.exists() && spec_path.is_file() {
            if let Ok(existing) = tokio::fs::read_to_string(&spec_path).await {
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
                    let diff = format!(
                        "{}",
                        pretty_assertions::StrComparison::new(
                            &normalized_existing,
                            &new_spec_pretty
                        )
                    );
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

        // Convert generated spec into ProjectSpec for subsequent API calls
        let project_spec: impulse_common::types::ProjectSpec =
            serde_json::from_value(serde_json::to_value(&gen_res.platform_spec.spec)?)?;
        tracing::info!("Spec: {:?}", project_spec);

        // Print compatibility report (colored comfy-table) if incompatible
        if !gen_res.compatibility_report.compatible
            && self.global_args.output_mode != OutputMode::Json
        {
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
        } else if self.global_args.output_mode == OutputMode::Json {
            // Defer printing; include in consolidated JSON
            if let Some(ref mut out) = json_out {
                out.compatibility = Some(gen_res.compatibility_report.clone());
            }
        }

        // Save start_command for build usage, .neptune/start_command
        let start_file = dir.join(".neptune").join("start_command");
        if let Some(parent) = start_file.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        tokio::fs::write(&start_file, gen_res.start_command.as_bytes()).await?;

        // Ask for confirmation (interactive only in non-JSON mode)
        if self.global_args.output_mode != OutputMode::Json {
            let proceed = Confirm::new()
                .with_prompt("Proceed with build and deployment?")
                .default(true)
                .interact()
                .unwrap_or(false);
            if !proceed {
                eprintln!("Aborted by user.");
                return Ok(NeptuneCommandOutput::None);
            }
        }

        // Build and push image (logs are streamed directly; no spinner)
        let build_result = match self.build(&project_spec.name, deploy_args).await {
            Ok(v) => v,
            Err(e) => {
                if self.global_args.output_mode != OutputMode::Json {
                    eprintln!("❌ Build failed - unable to create container image");
                    eprintln!("📋 Project: {}", project_spec.name);
                    eprintln!("🧾 Error: {}", e);
                    eprintln!("💡 Check build logs for errors and retry");
                } else if let Some(ref mut out) = json_out {
                    out.ok = false;
                    out.error = Some(ErrorPayload {
                        code: "BUILD_FAILED".to_string(),
                        message: "Build process failed to produce a container image".to_string(),
                        details: serde_json::json!({
                            "stage": "build",
                            "project_name": project_spec.name,
                            "raw_error": e.to_string(),
                        }),
                        next_action: Some("check_build_logs_then_retry".to_string()),
                        requires_confirmation: false,
                    });
                    // Print a single consolidated JSON error and exit
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
                return Ok(NeptuneCommandOutput::None);
            }
        };
        if let Some(image_name) = build_result {
            tracing::info!("Image name: {}", image_name);
            if let Some(ref mut out) = json_out {
                out.build = Some(BuildSummary {
                    image: Some(image_name.clone()),
                });
            }
            // Ensure project exists
            let ensure_spinner = if self.global_args.output_mode != OutputMode::Json {
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::with_template("{spinner:.green} {msg}")
                        .unwrap()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
                );
                pb.set_message("Ensuring project exists...");
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(pb)
            } else {
                None
            };
            let project = if let Some(project_id) = self
                .client
                .get_project_id_from_name(&project_spec.name)
                .await?
            {
                self.client.get_project_by_id(&project_id).await?
            } else {
                self.client.create_project(&project_spec).await?
            }
            .into_inner();
            if let Some(pb) = ensure_spinner.as_ref() {
                pb.finish_and_clear();
            }

            // Create deployment
            let deploy_spinner = if self.global_args.output_mode != OutputMode::Json {
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::with_template("{spinner:.green} {msg}")
                        .unwrap()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
                );
                pb.set_message("Creating deployment...");
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(pb)
            } else {
                None
            };
            let deployment = self
                .client
                .create_deployment(&project_spec, &project.id, &image_name)
                .await?
                .into_inner();
            if let Some(pb) = deploy_spinner.as_ref() {
                pb.finish_and_clear();
            }

            // Handle successful deployment output
            if self.global_args.output_mode == OutputMode::Json {
                if let Some(ref mut out) = json_out {
                    out.deployment = Some(DeploymentSummary {
                        project: project_spec.name.clone(),
                        deployment_id: deployment.id.clone(),
                        summary: "Deployment created. Monitoring status until ready.".to_string(),
                        messages: vec!["This may take a few minutes.".to_string()],
                        next_action: "poll_status".to_string(),
                        requires_confirmation: false,
                    });
                }
            } else {
                println!("✅ Deployment created.");
                println!("📦 Project: {}", project_spec.name);
                println!("🚀 Deployment ID: {}", deployment.id);
                println!("⏳ Monitoring status until the project is ready...");
            }
        }

        // Poll project status with per-dimension spinners (project/resources/workload)
        let (mp, pb_project, pb_resources, pb_workload) =
            if self.global_args.output_mode != OutputMode::Json {
                let mp = MultiProgress::new();
                let style = ProgressStyle::with_template("{spinner:.green} {msg}")
                    .unwrap()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
                let pb_project = mp.add(ProgressBar::new_spinner());
                pb_project.set_style(style.clone());
                pb_project.set_message("Project: waiting...");
                pb_project.enable_steady_tick(std::time::Duration::from_millis(80));
                let pb_resources = mp.add(ProgressBar::new_spinner());
                pb_resources.set_style(style.clone());
                pb_resources.set_message("Resources: waiting...");
                pb_resources.enable_steady_tick(std::time::Duration::from_millis(80));
                let pb_workload = mp.add(ProgressBar::new_spinner());
                pb_workload.set_style(style.clone());
                pb_workload.set_message("Workload: waiting...");
                pb_workload.enable_steady_tick(std::time::Duration::from_millis(80));
                (
                    Some(mp),
                    Some(pb_project),
                    Some(pb_resources),
                    Some(pb_workload),
                )
            } else {
                (None, None, None, None)
            };
        if let Some(project_id) = self
            .client
            .get_project_id_from_name(&project_spec.name)
            .await?
        {
            let mut attempts = 0u32;
            let max_attempts = 60u32; // ~5 min at 5s intervals
            let mut final_condition: Option<impulse_common::types::AggregateProjectCondition> =
                None;
            let mut proj_done = false;
            let mut res_done = false;
            let mut work_done = false;
            while attempts < max_attempts {
                let status = self
                    .client
                    .get_project_by_id(&project_id)
                    .await?
                    .into_inner();
                let cond = status.condition;
                let is_success = cond.project == ProjectState::Available
                    && matches!(cond.workload, WorkloadState::Running)
                    && cond.resources == ResourcesState::Available;
                let is_failure = matches!(cond.workload, WorkloadState::Failing(_))
                    || matches!(cond.resources, ResourcesState::Failing(_));

                // Update spinners with latest states
                if let (Some(pb_p), Some(pb_r), Some(pb_w)) = (
                    pb_project.as_ref(),
                    pb_resources.as_ref(),
                    pb_workload.as_ref(),
                ) {
                    let proj_text = format!("Project: {:?}", cond.project);
                    let res_text = match &cond.resources {
                        ResourcesState::Failing(msg) => format!("Resources: Failing - {}", msg),
                        other => format!("Resources: {:?}", other),
                    };
                    let work_text = match &cond.workload {
                        WorkloadState::Failing(msg) => format!("Workload: Failing - {}", msg),
                        other => format!("Workload: {:?}", other),
                    };
                    if !proj_done {
                        if cond.project == ProjectState::Available || is_success || is_failure {
                            pb_p.finish_with_message(format!("{} ✅", proj_text));
                            proj_done = true;
                        } else {
                            pb_p.set_message(proj_text);
                            pb_p.tick();
                        }
                    }
                    if !res_done {
                        match &cond.resources {
                            ResourcesState::Available | ResourcesState::Failing(_) => {
                                pb_r.finish_with_message(format!(
                                    "{} {}",
                                    res_text,
                                    if matches!(cond.resources, ResourcesState::Available) {
                                        "✅"
                                    } else {
                                        "❌"
                                    }
                                ));
                                res_done = true;
                            }
                            _ => {
                                pb_r.set_message(res_text);
                                pb_r.tick();
                            }
                        }
                    }
                    if !work_done {
                        match &cond.workload {
                            WorkloadState::Running | WorkloadState::Failing(_) => {
                                pb_w.finish_with_message(format!(
                                    "{} {}",
                                    work_text,
                                    if matches!(cond.workload, WorkloadState::Running) {
                                        "✅"
                                    } else {
                                        "❌"
                                    }
                                ));
                                work_done = true;
                            }
                            _ => {
                                pb_w.set_message(work_text);
                                pb_w.tick();
                            }
                        }
                    }
                }

                if is_success || is_failure {
                    final_condition = Some(cond);
                    break;
                }
                attempts += 1;
                sleep(Duration::from_secs(5)).await;
                final_condition = Some(cond);
            }
            // Ensure spinners are finished with their last known messages
            if let (Some(pb_p), Some(pb_r), Some(pb_w)) = (pb_project, pb_resources, pb_workload) {
                if !proj_done {
                    pb_p.finish_and_clear();
                }
                if !res_done {
                    pb_r.finish_and_clear();
                }
                if !work_done {
                    pb_w.finish_and_clear();
                }
            }
            if let Some(cond) = final_condition {
                if let Some(ref mut out) = json_out {
                    out.final_condition = Some(cond);
                    out.ok = true;
                }
            }
        }

        // Print consolidated JSON result at the end (JSON mode)
        if let Some(out) = json_out {
            println!("{}", serde_json::to_string_pretty(&out)?);
        }

        Ok(NeptuneCommandOutput::None)
    }
}
