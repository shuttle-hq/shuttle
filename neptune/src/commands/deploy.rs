use anyhow::{anyhow, Result};
use cargo_shuttle::args::OutputMode;
use chrono::Utc;
use dialoguer::Confirm;
use impulse_common::types::{ProjectState, ResourcesState, WorkloadState};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

use crate::{args::DeployArgs, ui::AiUi, Neptune, NeptuneCommandOutput};

use super::build::ArchiveType;
use super::common::{
    assess_lint_gate, generate_platform_spec, make_spinner, preview_spec_changes,
    print_ai_lint_report, write_start_command, SpecPreviewStatus,
};

#[derive(Serialize)]
struct BuildSummary {
    image: Option<String>,
}

#[derive(Serialize)]
struct DeploymentSummary {
    project: String,
    deployment_id: String,
    messages: Vec<String>,
}

#[derive(Serialize)]
struct DeployJsonOutput {
    ok: bool,
    project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
    next_action_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ai_lint_report")]
    ai_lint_report: Option<shuttle_api_client::neptune_types::AiLintReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<BuildSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deployment: Option<DeploymentSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_condition: Option<impulse_common::types::AggregateProjectCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_env: Option<BTreeMap<String, String>>,
}

fn docker_installed() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn docker_running() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl Neptune {
    pub async fn deploy(&self, deploy_args: DeployArgs) -> Result<NeptuneCommandOutput> {
        let project_name = self.resolve_project_name().await?;

        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("neptune.json");
        let mut _printed_up_to_date = false;

        // In JSON mode, we collect a single consolidated result to print at the end
        let mut json_out = if self.global_args.output_mode == OutputMode::Json {
            Some(DeployJsonOutput {
                ok: false,
                project: project_name.clone(),
                messages: None,
                next_action_command: String::new(),
                ai_lint_report: None,
                build: None,
                deployment: None,
                final_condition: None,
                final_url: None,
                final_env: None,
            })
        } else {
            None
        };

        let dir = &self.global_args.working_directory;
        let spec_path = dir.join("neptune.json");

        let project_spec: impulse_common::types::ProjectSpec;
        let ai_lint_report: shuttle_api_client::neptune_types::AiLintReport;
        let mut start_command: Option<String> = None;

        if deploy_args.skip_spec {
            if !spec_path.exists() {
                let message = format!(
                    "Cannot skip spec generation because {} does not exist",
                    spec_path.display()
                );
                if let Some(ref mut out) = json_out {
                    out.ok = false;
                    out.messages = Some(vec![
                        message.clone(),
                        "Run `neptune generate spec` first to create the file.".to_string(),
                    ]);
                    out.next_action_command = "neptune generate spec".to_string();
                    println!("{}", serde_json::to_string_pretty(&out)?);
                    return Ok(NeptuneCommandOutput::None);
                } else {
                    return Err(anyhow!(message));
                }
            }

            let existing_spec_bytes = tokio::fs::read(&spec_path).await?;
            let stored_spec: serde_json::Value = serde_json::from_slice(&existing_spec_bytes)?;
            project_spec = serde_json::from_value(stored_spec)?;

            let lint_spinner = make_spinner(&self.global_args.output_mode, "Running AI lint...");
            let bytes: Vec<u8> = self.create_build_context(
                dir,
                ArchiveType::Zip,
                None::<Vec<&std::path::Path>>,
                true,
            )?;
            let lint_res = match self.client.ai_lint(bytes).await {
                Ok(report) => report,
                Err(e) => {
                    if let Some(pb) = lint_spinner.as_ref() {
                        pb.finish_and_clear();
                    }
                    if let Some(ref mut out) = json_out {
                        out.ok = false;
                        out.messages = Some(vec![
                            "Failed to analyze project with AI lint".to_string(),
                            format!("Project: {}", project_name),
                            format!("Error: {}", e),
                        ]);
                        out.next_action_command = "neptune deploy --skip-spec".to_string();
                        println!("{}", serde_json::to_string_pretty(&out)?);
                        return Ok(NeptuneCommandOutput::None);
                    } else {
                        return Err(e);
                    }
                }
            };
            if let Some(pb) = lint_spinner.as_ref() {
                pb.finish_and_clear();
            }

            ai_lint_report = lint_res;
        } else {
            let gen_spinner = make_spinner(
                &self.global_args.output_mode,
                "Analyzing project and generating configuration...",
            );

            let gen_res = match generate_platform_spec(
                self,
                &self.global_args.working_directory,
                &project_name,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    if let Some(ref mut out) = json_out {
                        out.ok = false;
                        out.messages = Some(vec![
                            "Failed to analyze project and generate configuration".to_string(),
                            format!("Project: {}", project_name),
                            format!("Error: {}", e),
                        ]);
                        out.next_action_command = "neptune deploy".to_string();
                        println!("{}", serde_json::to_string_pretty(&out)?);
                        return Ok(NeptuneCommandOutput::None);
                    } else {
                        return Err(e);
                    }
                }
            };

            if let Some(pb) = gen_spinner.as_ref() {
                pb.finish_and_clear();
            }

            let new_spec_pretty = serde_json::to_string_pretty(&gen_res.platform_spec)?;
            match preview_spec_changes(
                &spec_path,
                &new_spec_pretty,
                &ui,
                &self.global_args.output_mode,
            )? {
                SpecPreviewStatus::UpToDate => _printed_up_to_date = true,
                _ => {}
            };

            tokio::fs::write(&spec_path, new_spec_pretty.as_bytes()).await?;

            project_spec = serde_json::from_value(serde_json::to_value(&gen_res.platform_spec)?)?;
            ai_lint_report = gen_res.ai_lint_report.clone();
            start_command = Some(gen_res.start_command.clone());
        }
        tracing::info!("Spec: {:?}", project_spec);

        if let Some(cmd) = start_command {
            write_start_command(dir, &cmd).await?;
        }

        if self.global_args.output_mode == OutputMode::Json {
            if let Some(ref mut out) = json_out {
                out.ai_lint_report = Some(ai_lint_report.clone());
            }
        } else {
            print_ai_lint_report(&ui, &ai_lint_report, &self.global_args.output_mode);
        }

        let lint_assessment = assess_lint_gate(
            &ai_lint_report,
            self.global_args.allow_ai_errors,
            self.global_args.allow_ai_warnings,
        );
        if lint_assessment.blocking {
            if let Some(mut out) = json_out.take() {
                let mut msgs = out.messages.unwrap_or_default();
                msgs.extend(lint_assessment.reasons.clone());
                msgs.push("Deployment aborted before build due to AI lint findings.".to_string());
                out.messages = Some(msgs);
                out.ok = false;
                if out.ai_lint_report.is_none() {
                    out.ai_lint_report = Some(ai_lint_report.clone());
                }
                out.next_action_command = "neptune lint".to_string();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                for reason in &lint_assessment.reasons {
                    ui.warn(format!("Blocking: {}", reason));
                }
                ui.info("Use --allow-ai-errors / --allow-ai-warnings to override.");
                ui.info("Deployment aborted before build; resolve findings or override to continue.");
            }
            return Ok(NeptuneCommandOutput::None);
        }

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

        let ensure_spinner =
            make_spinner(&self.global_args.output_mode, "Ensuring project exists...");
        let project = match async {
            if let Some(project_id) = self
                .client
                .get_project_id_from_name(&project_spec.name)
                .await?
            {
                self.client.get_project_by_id(&project_id).await
            } else {
                self.client.create_project(&project_spec).await
            }
        }
        .await
        {
            Ok(resp) => resp.into_inner(),
            Err(e) => {
                if let Some(ref mut out) = json_out {
                    out.ok = false;
                    out.messages = Some(vec![
                        "Failed to ensure project exists".to_string(),
                        format!("Project: {}", project_spec.name),
                        format!("Error: {}", e),
                    ]);
                    out.next_action_command = "neptune deploy".to_string();
                    println!("{}", serde_json::to_string_pretty(&out)?);
                    if let Some(pb) = ensure_spinner.as_ref() {
                        pb.finish_and_clear();
                    }
                    return Ok(NeptuneCommandOutput::None);
                } else {
                    if let Some(pb) = ensure_spinner.as_ref() {
                        pb.finish_and_clear();
                    }
                    return Err(e.into());
                }
            }
        };
        if let Some(pb) = ensure_spinner.as_ref() {
            pb.finish_and_clear();
        }

        // Preflight: Docker availability
        ui.header("Preflight");
        if !docker_installed() {
            let install_link = "https://docs.docker.com/get-docker/";
            if self.global_args.output_mode != OutputMode::Json {
                ui.warn("Docker is not installed or not found in PATH");
                ui.info(format!("Install Docker: {}", install_link));
            } else if let Some(ref mut out) = json_out {
                out.ok = false;
                out.messages = Some(vec![
                    "Docker is not installed or not found in PATH".to_string(),
                    format!("Install Docker: {}", install_link),
                ]);
                out.next_action_command = "neptune deploy".to_string();
                println!("{}", serde_json::to_string_pretty(&out)?);
            }
            return Ok(NeptuneCommandOutput::None);
        }
        if !docker_running() {
            let help_msg =
                "Docker daemon is not running. Start Docker (e.g., Docker Desktop) and retry.";
            if self.global_args.output_mode != OutputMode::Json {
                ui.warn(help_msg);
                ui.info("If you don't have Docker yet, install it from: https://docs.docker.com/get-docker/");
            } else if let Some(ref mut out) = json_out {
                out.ok = false;
                out.messages = Some(vec![
                    help_msg.to_string(),
                    "If you don't have Docker yet, install it from: https://docs.docker.com/get-docker/".to_string(),
                ]);
                out.next_action_command = "neptune deploy".to_string();
                println!("{}", serde_json::to_string_pretty(&out)?);
            }
            return Ok(NeptuneCommandOutput::None);
        }
        ui.step("🐳", "Docker is installed and running");

        // Build and push image (logs are streamed directly; no spinner)
        ui.header("Build");
        let build_result = match self.build(&project.id, deploy_args).await {
            Ok(v) => v,
            Err(e) => {
                if self.global_args.output_mode != OutputMode::Json {
                    ui.warn("❌ Build failed - unable to create container image");
                    ui.step("", format!("Project: {}", project_spec.name));
                    ui.step("", format!("Error: {}", e));
                    ui.step("", "Check build logs for errors and retry");
                } else if let Some(ref mut out) = json_out {
                    out.ok = false;
                    out.messages = Some(vec![
                        "Build failed - unable to create container image".to_string(),
                        format!("Project: {}", project_spec.name),
                        format!("Error: {}", e),
                        "Check build logs for errors and retry".to_string(),
                    ]);
                    out.next_action_command = "neptune deploy".to_string();
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
            ui.header("Deploy");
            // Create deployment
            let deploy_spinner =
                make_spinner(&self.global_args.output_mode, "Creating deployment...");
            let deployment = match self
                .client
                .create_deployment(&project_spec, &project.id, &image_name)
                .await
            {
                Ok(resp) => resp.into_inner(),
                Err(e) => {
                    if let Some(ref mut out) = json_out {
                        out.ok = false;
                        out.messages = Some(vec![
                            "Failed to create deployment".to_string(),
                            format!("Project: {}", project_spec.name),
                            format!("Error: {}", e),
                        ]);
                        out.next_action_command = "neptune deploy".to_string();
                        println!("{}", serde_json::to_string_pretty(&out)?);
                        if let Some(pb) = deploy_spinner.as_ref() {
                            pb.finish_and_clear();
                        }
                        return Ok(NeptuneCommandOutput::None);
                    } else {
                        if let Some(pb) = deploy_spinner.as_ref() {
                            pb.finish_and_clear();
                        }
                        return Err(e.into());
                    }
                }
            };
            if let Some(pb) = deploy_spinner.as_ref() {
                pb.finish_and_clear();
            }

            // Handle successful deployment output
            if self.global_args.output_mode == OutputMode::Json {
                if let Some(ref mut out) = json_out {
                    out.deployment = Some(DeploymentSummary {
                        project: project_spec.name.clone(),
                        deployment_id: deployment.id.clone(),
                        messages: vec![
                            format!("Deployment created at {}", Utc::now().to_string()),
                            "This may take a few minutes.".to_string(),
                        ],
                    });
                    out.next_action_command = "neptune status".to_string();
                }
            } else {
                ui.success("✅ Deployment created");
                ui.step("", format!("Project: {}", project_spec.name));
                ui.step("", format!("Deployment ID: {}", deployment.id));
                ui.step("", "Monitoring status until the project is ready...");
            }
        }

        // Poll project status with per-dimension spinners (project/resources/workload)
        ui.header("Status");
        let (_mp, pb_project, pb_resources, pb_workload) =
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
            let mut final_url: Option<String> = None;
            let mut final_env: Option<BTreeMap<String, String>> = None;
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
                final_url = status.url.clone();
                final_env = status.env.clone();
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
                    out.final_url = final_url;
                    out.final_env = final_env.clone();
                    out.ok = true;
                } else {
                    if let Some(url) = final_url {
                        ui.step("", format!("URL: {}", url));
                    }
                    if let Some(env) = final_env {
                        if !env.is_empty() {
                            ui.step("", "Environment variables:");
                            for (k, v) in env {
                                ui.step("", format!("{}={}", k, v));
                            }
                        }
                    }
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
