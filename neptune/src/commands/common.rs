use std::path::Path;
use std::{fs, time::Duration};

use anyhow::Result;
use cargo_shuttle::args::OutputMode;
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use indicatif::{ProgressBar, ProgressStyle};
use pretty_assertions::StrComparison;
use shuttle_api_client::neptune_types::{CompatibilityReport, GenerateResponse};

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
    let bytes: Vec<u8> =
        neptune.create_build_context(dir, ArchiveType::Zip, None::<Vec<&Path>>, true)?;
    let gen_res = neptune.client.generate(bytes, project_name).await?;
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

/// Print a compatibility report table if incompatible and not in JSON mode.
pub fn print_compatibility_report_if_needed(
    ui: &AiUi,
    report: &CompatibilityReport,
    output_mode: &OutputMode,
) {
    if *output_mode == OutputMode::Json || report.compatible {
        return;
    }
    eprintln!();
    ui.header("Compatibility Report");
    eprintln!();
    ui.warn("Possible compatibility issues detected");
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
    for err in report.errors.iter() {
        let category = match err.category {
            shuttle_api_client::neptune_types::ErrorCategory::Architecture => "Architecture",
            shuttle_api_client::neptune_types::ErrorCategory::ResourceSupport => "Resource Support",
            shuttle_api_client::neptune_types::ErrorCategory::WorkloadSupport => "Workload Support",
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
}
