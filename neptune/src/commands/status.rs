use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use impulse_common::types::{ProjectState, ResourcesState, WorkloadState};
use serde_json::Value;

use crate::{ui::AiUi, Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn status(&self) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Status");

        // Determine project name: prefer neptune.json's spec.name, fallback to working directory name
        let dir = &self.global_args.working_directory;
        let spec_path = dir.join("neptune.json");
        let mut project_name = self
            .global_args
            .workdir_name()
            .unwrap_or_else(|| "project".to_string());
        if spec_path.exists() && spec_path.is_file() {
            if let Ok(content) = tokio::fs::read_to_string(&spec_path).await {
                if let Ok(v) = serde_json::from_str::<Value>(&content) {
                    if let Some(name) = v
                        .get("spec")
                        .and_then(|s| s.get("name"))
                        .and_then(|n| n.as_str())
                    {
                        project_name = name.to_string();
                    }
                }
            }
        }

        // Look up project on Shuttle by name
        if let Some(project_id) = self.client.get_project_id_from_name(&project_name).await? {
            let status = self
                .client
                .get_project_by_id(&project_id)
                .await?
                .into_inner()
                .condition;
            if self.global_args.output_mode == OutputMode::Json {
                println!("{}", serde_json::to_string_pretty(&status)?);
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
            }
            Ok(NeptuneCommandOutput::None)
        } else {
            if self.global_args.output_mode == OutputMode::Json {
                eprintln!(
                    indoc::indoc! {r#"
                    {{
                        "error": "project_not_found",
                        "message": "The project '{}' was not found on the Shuttle platform",
                        "suggestion": "Run 'neptune deploy' to create and deploy this project to Shuttle",
                        "next_action": "deploy_project",
                        "project_name": "{}"
                    }}"#
                    },
                    project_name, project_name
                );
            } else {
                ui.warn("Project not found");
                ui.step("", format!("Project: {}", project_name));
                ui.info("Run 'neptune deploy' to build and deploy this project");
            }
            Ok(NeptuneCommandOutput::None)
        }
    }
}
