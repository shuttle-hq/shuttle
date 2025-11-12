use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use impulse_common::types::{ProjectState, ResourcesState, WorkloadState};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::{args::StatusArgs, ui::AiUi, Neptune, NeptuneCommandOutput};

#[derive(Serialize)]
struct StatusJsonOutput {
    ok: bool,
    project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition: Option<impulse_common::types::AggregateProjectCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<BTreeMap<String, String>>,
    next_action_command: String,
}

impl Neptune {
    pub async fn status(&self, status_args: StatusArgs) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Status");

        let project_name = if let Some(name) = status_args.project_name {
            name
        } else {
            self.resolve_project_name().await?
        };

        // Prepare consolidated JSON output if in JSON mode
        let mut json_out = if self.global_args.output_mode == OutputMode::Json {
            Some(StatusJsonOutput {
                ok: false,
                project: project_name.clone(),
                messages: None,
                condition: None,
                url: None,
                env: None,
                next_action_command: String::new(),
            })
        } else {
            None
        };

        // Look up project on Platform by name
        if let Some(project_id) = self.client.get_project_id_from_name(&project_name).await? {
            let response = self
                .client
                .get_project_by_id(&project_id)
                .await?
                .into_inner();
            let impulse_common::types::ProjectStatusResponse {
                condition: status,
                url,
                env,
                ..
            } = response;
            if let Some(ref mut out) = json_out {
                out.ok = true;
                out.condition = Some(status);
                out.url = url.clone();
                out.env = env.clone();
                out.next_action_command = "neptune status".to_string();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                ui.step("", format!("Project: {}", project_name));
                // Project
                match &status.project {
                    ProjectState::Available => ui.success("✅ Project: Available"),
                    ProjectState::Created => ui.success("✅ Project: Created"),
                    other => ui.step("", format!("Project: {:?}", other)),
                }
                // Resources
                match &status.resources {
                    ResourcesState::Available => ui.success("✅ Resources: Available"),
                    ResourcesState::Failing(msg) => {
                        ui.warn(format!("Resources: Failing - {}", msg))
                    }
                    other => ui.step("", format!("Resources: {:?}", other)),
                }
                // Workload
                match &status.workload {
                    WorkloadState::Running => ui.success("✅ Workload: Running"),
                    WorkloadState::Failing(msg) => ui.warn(format!("Workload: Failing - {}", msg)),
                    other => ui.step("", format!("Workload: {:?}", other)),
                }
                // Overall summary
                if matches!(
                    status.project,
                    ProjectState::Available | ProjectState::Created
                ) && matches!(status.resources, ResourcesState::Available)
                    && matches!(status.workload, WorkloadState::Running)
                {
                    ui.success("✅ All systems operational");
                }
                if let Some(url) = url {
                    ui.step("", format!("URL: {}", url));
                }
                if let Some(env) = env {
                    if !env.is_empty() {
                        ui.step("", "Environment variables:");
                        for (k, v) in env {
                            ui.step("", format!("{}={}", k, v));
                        }
                    }
                }
            }
            Ok(NeptuneCommandOutput::None)
        } else {
            if let Some(ref mut out) = json_out {
                out.ok = false;
                out.messages = Some(vec![
                    "Project not found".to_string(),
                    format!("Project: {}", project_name),
                    "Run 'neptune deploy' to create and deploy this project to Shuttle".to_string(),
                ]);
                out.next_action_command = "neptune deploy".to_string();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                ui.warn("Project not found");
                ui.step("", format!("Project: {}", project_name));
                ui.info("Run 'neptune deploy' to build and deploy this project");
            }
            Ok(NeptuneCommandOutput::None)
        }
    }
}
