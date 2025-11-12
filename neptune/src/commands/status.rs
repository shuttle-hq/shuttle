use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use impulse_common::types::{ProjectState, ResourcesState, WorkloadState};

use crate::{ui::AiUi, Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn status(&self) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Status");

        let project_name = self.resolve_project_name().await?;

        // Look up project on Platform by name
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
