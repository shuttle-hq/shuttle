use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use dialoguer::Input;

use crate::{ui::AiUi, Neptune, NeptuneCommandOutput};

use super::common::make_spinner;
use serde::Serialize;

#[derive(Serialize)]
struct DeleteJsonOutput {
    ok: bool,
    project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
    next_action_command: String,
}

impl Neptune {
    pub async fn delete(&self) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Delete project");

        let project_name = self.resolve_project_name().await?;
        ui.step("", format!("Project: {}", project_name));

        // Prepare consolidated JSON output if in JSON mode
        let mut json_out = if self.global_args.output_mode == OutputMode::Json {
            Some(DeleteJsonOutput {
                ok: false,
                project: project_name.clone(),
                messages: None,
                next_action_command: String::new(),
            })
        } else {
            None
        };

        let project_id_opt = self.client.get_project_id_from_name(&project_name).await?;
        let Some(project_id) = project_id_opt else {
            if let Some(ref mut out) = json_out {
                out.ok = false;
                out.messages = Some(vec![
                    "Project not found".to_string(),
                    format!("Project: {}", project_name),
                    "Check project name is correct and try again".to_string(),
                ]);
                out.next_action_command = "neptune delete".to_string();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                ui.warn("Project not found");
            }
            return Ok(NeptuneCommandOutput::None);
        };

        if self.global_args.output_mode != OutputMode::Json {
            let confirmation: String = Input::new()
                .with_prompt("Type 'delete' to confirm project deletion")
                .interact_text()?;
            if confirmation.trim() != "delete" {
                ui.warn("Aborted: confirmation text did not match 'delete'");
                return Ok(NeptuneCommandOutput::None);
            }
        }

        let spinner = make_spinner(&self.global_args.output_mode, "Deleting project...");

        let res = self.client.delete_project_by_id(&project_id).await;

        if let Some(pb) = spinner.as_ref() {
            pb.finish_and_clear();
        }

        match res {
            Ok(_) => {
                if let Some(ref mut out) = json_out {
                    out.ok = true;
                    out.messages = Some(vec![format!("Project '{}' deleted", project_name)]);
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    ui.success(format!("✅ Project '{}' deleted", project_name));
                }
                Ok(NeptuneCommandOutput::None)
            }
            Err(e) => {
                if let Some(ref mut out) = json_out {
                    out.ok = false;
                    out.messages = Some(vec![
                        "Failed to delete project".to_string(),
                        format!("Project: {}", project_name),
                        format!("Error: {}", e),
                    ]);
                    out.next_action_command = "neptune delete".to_string();
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    ui.warn("Failed to delete project");
                    ui.step("", format!("Error: {}", e));
                }
                Ok(NeptuneCommandOutput::None)
            }
        }
    }
}
