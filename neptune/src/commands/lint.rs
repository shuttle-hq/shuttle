use std::path::Path;

use anyhow::{anyhow, Result};
use cargo_shuttle::args::OutputMode;
use serde::Serialize;
use serde_json::Value;

use crate::{ui::AiUi, Neptune, NeptuneCommandOutput};

use super::{
    build::ArchiveType,
    common::{assess_lint_gate, make_spinner, print_ai_lint_report},
};

#[derive(Serialize)]
struct LintJsonOutput {
    ok: bool,
    project: String,
    #[serde(rename = "ai_lint_report")]
    ai_lint_report: shuttle_api_client::neptune_types::AiLintReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
    next_action_command: String,
}

impl Neptune {
    pub async fn lint(&self) -> Result<NeptuneCommandOutput> {
        let project_name = self.resolve_project_name().await.unwrap_or_else(|_| {
            self.global_args
                .workdir_name()
                .unwrap_or_else(|| "project".to_string())
        });

        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        if self.global_args.output_mode != OutputMode::Json {
            ui.header("AI Lint");
        }

        let spec_path = self.global_args.working_directory.join("neptune.json");
        if !spec_path.exists() {
            let missing_spec_message = format!(
                "Missing {}. Run `neptune generate spec` before linting.",
                spec_path.display()
            );
            if self.global_args.output_mode == OutputMode::Json {
                let payload = serde_json::json!({
                    "ok": false,
                    "project": project_name.clone(),
                    "ai_lint_report": Value::Null,
                    "messages": [missing_spec_message.clone()],
                    "next_action_command": "neptune generate spec",
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                ui.warn("neptune.json not found in the current workspace");
                ui.info(format!("Expected to find {}", spec_path.display()));
                ui.info("Run `neptune generate spec` to create it before linting.");
            }
            return Err(anyhow!(missing_spec_message));
        }

        let spinner = make_spinner(
            &self.global_args.output_mode,
            "Analyzing project with AI lint...",
        );
        let bytes: Vec<u8> = self.create_build_context(
            &self.global_args.working_directory,
            ArchiveType::Zip,
            None::<Vec<&Path>>,
            true,
        )?;
        let report = match self.client.ai_lint(bytes).await {
            Ok(report) => report,
            Err(e) => {
                if let Some(pb) = spinner.as_ref() {
                    pb.finish_and_clear();
                }
                return Err(e);
            }
        };
        if let Some(pb) = spinner.as_ref() {
            pb.finish_and_clear();
        }

        let assessment = assess_lint_gate(
            &report,
            self.global_args.allow_ai_errors,
            self.global_args.allow_ai_warnings,
        );
        let failure_message = assessment.blocking.then(|| {
            format!(
                "AI lint reported blocking findings: {}",
                assessment.reasons.join("; ")
            )
        });

        if self.global_args.output_mode == OutputMode::Json {
            let mut out = LintJsonOutput {
                ok: !assessment.blocking,
                project: project_name,
                ai_lint_report: report.clone(),
                messages: None,
                next_action_command: if assessment.blocking {
                    "neptune lint".to_string()
                } else {
                    "neptune deploy".to_string()
                },
            };
            if assessment.blocking {
                out.messages = Some(assessment.reasons.clone());
            }
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            print_ai_lint_report(&ui, &report, &self.global_args.output_mode);
            if assessment.blocking {
                for reason in &assessment.reasons {
                    ui.warn(format!("Blocking: {}", reason));
                }
                ui.info("Use --allow-ai-errors / --allow-ai-warnings to override.");
            } else {
                ui.success("✅ No blocking AI lint findings");
            }
        }

        if let Some(message) = failure_message {
            return Err(anyhow!(message));
        }

        Ok(NeptuneCommandOutput::None)
    }
}
