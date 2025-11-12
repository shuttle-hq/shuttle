use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use dialoguer::Input;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::{ui::AiUi, Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn delete(&self) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Delete project");

        let project_name = self.resolve_project_name().await?;
        ui.step("", format!("Project: {}", project_name));

        let project_id_opt = self.client.get_project_id_from_name(&project_name).await?;
        let Some(project_id) = project_id_opt else {
            if self.global_args.output_mode != OutputMode::Json {
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

        let spinner = if self.global_args.output_mode != OutputMode::Json {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("{spinner:.green} {msg}")
                    .unwrap()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
            );
            pb.set_message("Deleting project...");
            pb.enable_steady_tick(Duration::from_millis(80));
            Some(pb)
        } else {
            None
        };

        let res = self.client.delete_project_by_id(&project_id).await;

        if let Some(pb) = spinner.as_ref() {
            pb.finish_and_clear();
        }

        match res {
            Ok(_) => {
                if self.global_args.output_mode != OutputMode::Json {
                    ui.success(format!("✅ Project '{}' deleted", project_name));
                }
                Ok(NeptuneCommandOutput::None)
            }
            Err(e) => {
                if self.global_args.output_mode != OutputMode::Json {
                    ui.warn("Failed to delete project");
                    ui.step("", format!("Error: {}", e));
                }
                Err(e)
            }
        }
    }
}
