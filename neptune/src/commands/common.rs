use std::path::Path;
use std::{fs, time::Duration};

use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use indicatif::{ProgressBar, ProgressStyle};
use pretty_assertions::StrComparison;
use shuttle_api_client::neptune_types::{
    AiLintCategory, AiLintFinding, AiLintReport, GenerateResponse,
};

use crate::{ui::AiUi, Neptune};

use super::build::ArchiveType;

/// Create a standard spinner used across commands, or None in JSON mode.
pub fn make_spinner(output_mode: &OutputMode, message: &str) -> Option<ProgressBar> {
    if *output_mode == OutputMode::Json {
        return None;
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    Some(pb)
}

/// Generate the platform spec by archiving the given directory and invoking the API.
pub async fn generate_platform_spec(
    neptune: &Neptune,
    dir: &Path,
    project_name: &str,
) -> Result<GenerateResponse> {
    let project_archive: Vec<u8> =
        neptune.create_build_context(dir, ArchiveType::Zip, None::<Vec<&Path>>, true)?;
    let mut gen_res = neptune
        .client
        .generate(project_archive.clone(), project_name)
        .await?;

    if gen_res.ai_lint_report.is_none() {
        tracing::warn!(
            "AI lint report missing from /v1/generate response, running dedicated lint request"
        );
        let lint_report = neptune.client.ai_lint(project_archive).await?;
        gen_res.ai_lint_report = Some(lint_report);
    }

    Ok(gen_res)
}

/// Status of spec preview after comparing existing spec with the new one.
pub enum SpecPreviewStatus {
    UpToDate,
    CreatedPreview,
    UpdatedPreview,
    SkippedJsonMode,
}

/// Print a small diff/preview and status for neptune.json without writing the file.
pub fn preview_spec_changes(
    spec_path: &Path,
    new_spec_pretty: &str,
    ui: &AiUi,
    output_mode: &OutputMode,
) -> Result<SpecPreviewStatus> {
    if *output_mode == OutputMode::Json {
        return Ok(SpecPreviewStatus::SkippedJsonMode);
    }

    if spec_path.exists() && spec_path.is_file() {
        if let Ok(existing) = fs::read_to_string(spec_path) {
            // Normalize existing to pretty JSON to avoid whitespace-only diffs
            let normalized_existing = serde_json::from_str::<serde_json::Value>(&existing)
                .ok()
                .and_then(|v| serde_json::to_string_pretty(&v).ok())
                .unwrap_or(existing.clone());
            let changed = normalized_existing != new_spec_pretty;
            if !changed {
                ui.success("✅ neptune.json up to date");
                return Ok(SpecPreviewStatus::UpToDate);
            }
            ui.step("", "Updating neptune.json...");
            eprintln!();
            eprintln!("--- neptune.json ---");
            let diff = format!(
                "{}",
                StrComparison::new(&normalized_existing, new_spec_pretty)
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
            eprintln!("(Tip: run `git --no-pager diff -- neptune.json` to see full changes)");
            return Ok(SpecPreviewStatus::UpdatedPreview);
        }
    }

    ui.step("", "Creating neptune.json...");
    eprintln!();
    eprintln!("--- neptune.json ---");
    let diff = format!("{}", StrComparison::new("", new_spec_pretty));
    let max_lines = 60usize;
    for (i, line) in diff.lines().enumerate() {
        if i >= max_lines {
            eprintln!("... (truncated preview)");
            break;
        }
        eprintln!("{}", line);
    }
    eprintln!();
    eprintln!("(Tip: run `git --no-pager diff -- neptune.json` after saving to see full changes)");

    Ok(SpecPreviewStatus::CreatedPreview)
}

/// Write the generated start command to .neptune/start_command
pub async fn write_start_command(root_dir: &Path, start_command: &str) -> Result<()> {
    let start_file = root_dir.join(".neptune").join("start_command");
    if let Some(parent) = start_file.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    tokio::fs::write(&start_file, start_command.as_bytes()).await?;
    Ok(())
}

/// Render the AI lint report in human-readable form when not in JSON mode.
pub fn print_ai_lint_report(ui: &AiUi, report: &AiLintReport, output_mode: &OutputMode) {
    if *output_mode == OutputMode::Json {
        return;
    }
    if report.errors.is_empty() && report.warnings.is_empty() && report.suppressed.is_empty() {
        return;
    }
    eprintln!();
    ui.header("AI Lint Report");
    eprintln!();
    if report.summary.blocking {
        ui.warn("Blocking AI lint findings detected");
    } else {
        ui.step("", "No blocking findings detected");
    }
    render_section(ui, "Errors", &report.errors, Color::Red);
    render_section(ui, "Warnings", &report.warnings, Color::Yellow);
    render_section(ui, "Suppressed", &report.suppressed, Color::Cyan);
    if report.config.block_on_warnings {
        ui.step(
            "",
            "Repo config is set to block on warnings (block_on_warnings = true)",
        );
    }
    if !report.config.suppressed_codes.is_empty() {
        ui.step(
            "",
            format!(
                "Suppressed rules: {}",
                report.config.suppressed_codes.join(", ")
            ),
        );
    }
    eprintln!();
}

fn render_section(ui: &AiUi, label: &str, findings: &[AiLintFinding], color: Color) {
    if findings.is_empty() {
        return;
    }
    ui.step("", format!("{label} ({})", findings.len()));
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Category").add_attribute(Attribute::Bold),
            Cell::new("Code").add_attribute(Attribute::Bold),
            Cell::new("Message").add_attribute(Attribute::Bold),
            Cell::new("Path").add_attribute(Attribute::Bold),
            Cell::new("Suggestion").add_attribute(Attribute::Bold),
        ]);
    for finding in findings {
        table.add_row(vec![
            Cell::new(category_label(&finding.category)),
            Cell::new(finding.code.as_str()),
            Cell::new(finding.message.as_str()).fg(color),
            Cell::new(finding.path.as_deref().unwrap_or("-")),
            Cell::new(finding.suggestion.as_deref().unwrap_or("-")),
        ]);
    }
    eprintln!("{}", table);
    eprintln!();
}

fn category_label(category: &AiLintCategory) -> &'static str {
    match category {
        AiLintCategory::Architecture => "Architecture",
        AiLintCategory::ResourceSupport => "Resource Support",
        AiLintCategory::WorkloadSupport => "Workload Support",
        AiLintCategory::ConfigurationInvalid => "Configuration Invalid",
        AiLintCategory::Unknown => "Other",
    }
}

#[derive(Debug, Default, Clone)]
pub struct LintGateAssessment {
    pub blocking: bool,
    pub reasons: Vec<String>,
}

pub fn assess_lint_gate(
    report: &AiLintReport,
    allow_ai_errors: bool,
    allow_ai_warnings: bool,
) -> LintGateAssessment {
    let mut reasons = Vec::new();
    let error_count = report.errors.len();
    if error_count > 0 && !allow_ai_errors {
        reasons.push(format!(
            "{error_count} blocking error{} reported by AI lint",
            if error_count == 1 { "" } else { "s" }
        ));
    }
    let warning_count = report.warnings.len();
    if report.config.block_on_warnings && warning_count > 0 && !allow_ai_warnings {
        reasons.push(format!(
            "{warning_count} warning{} with block_on_warnings enabled",
            if warning_count == 1 { "" } else { "s" }
        ));
    }

    LintGateAssessment {
        blocking: !reasons.is_empty(),
        reasons,
    }
}
