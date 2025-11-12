use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use impulse_common::types::{
    AggregateProjectCondition, ProjectState, ProjectStatusResponse, ResourcesState, WorkloadState,
};
use serde::Serialize;

use crate::{
    args::{ListArgs, ListWhat},
    ui::AiUi,
    Neptune, NeptuneCommandOutput,
};

use super::common::make_spinner;

#[derive(Serialize)]
struct ListJsonOutput {
    ok: bool,
    projects: Vec<ProjectStatusResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
    next_action_command: String,
}

impl Neptune {
    pub async fn list(&self, list_args: ListArgs) -> Result<NeptuneCommandOutput> {
        match list_args.what {
            ListWhat::Projects => self.list_projects().await,
        }
    }

    async fn list_projects(&self) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Projects");

        // Fetch with spinner and error handling
        let spinner = make_spinner(&self.global_args.output_mode, "Fetching projects...");
        let projects_res = self.client.get_projects().await;
        if let Some(pb) = spinner.as_ref() {
            pb.finish_and_clear();
        }
        let projects = match projects_res {
            Ok(resp) => resp.into_inner(),
            Err(e) => {
                if self.global_args.output_mode == OutputMode::Json {
                    let out = ListJsonOutput {
                        ok: false,
                        projects: vec![],
                        messages: Some(vec![
                            "Failed to fetch projects".to_string(),
                            format!("Error: {}", e),
                        ]),
                        next_action_command: "neptune list projects".to_string(),
                    };
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    ui.warn("Failed to fetch projects");
                    ui.step("", format!("Error: {}", e));
                    ui.info(
                        "Try again later or run 'neptune list projects --output json' for details",
                    );
                }
                return Ok(NeptuneCommandOutput::None);
            }
        };

        if self.global_args.output_mode == OutputMode::Json {
            let out = ListJsonOutput {
                ok: true,
                projects,
                messages: None,
                next_action_command: "neptune status --project-name <name>".to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(NeptuneCommandOutput::None);
        }

        if projects.is_empty() {
            ui.info("No projects found");
            ui.info("Run 'neptune deploy' to create and deploy your first project");
            return Ok(NeptuneCommandOutput::None);
        }

        for p in projects {
            // Basic details
            eprintln!();
            ui.step("", format!("Project: {}", p.name));
            ui.step("", format!("  ID: {}", p.id));
            ui.step("", format!("  Kind: {:?}", p.kind));
            ui.step("", format!("  Resources: {}", p.resources.len()));
            if let Some(url) = &p.url {
                ui.step("", format!("  URL: {}", url));
            } else {
                ui.step("", "  URL: -");
            }
            if let Some(env) = &p.env {
                ui.step("", format!("  Env vars: {}", env.len()));
            } else {
                ui.step("", "  Env vars: 0");
            }
            // Condition overview
            Self::print_condition_overview(&ui, &p.condition);
            ui.step(
                "",
                format!(
                    "  Tip: run 'neptune status --project-name {}' for details",
                    p.name
                ),
            );
        }
        eprintln!();
        Ok(NeptuneCommandOutput::None)
    }

    fn print_condition_overview(ui: &AiUi, cond: &AggregateProjectCondition) {
        match cond.project {
            ProjectState::Available => ui.success("  ✅ Project: Available"),
            ProjectState::Created => ui.success("  ✅ Project: Created"),
            other => ui.step("", format!("  Project: {:?}", other)),
        }
        match &cond.resources {
            ResourcesState::Available => ui.success("  ✅ Resources: Available"),
            ResourcesState::Failing(msg) => ui.warn(format!("  Resources: Failing - {}", msg)),
            other => ui.step("", format!("  Resources: {:?}", other)),
        }
        match &cond.workload {
            WorkloadState::Running => ui.success("  ✅ Workload: Running"),
            WorkloadState::Failing(msg) => ui.warn(format!("Workload: Failing - {}", msg)),
            other => ui.step("", format!("  Workload: {:?}", other)),
        }
    }
}
