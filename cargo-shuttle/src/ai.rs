use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use std::fs;
use std::path::Path;

use crate::args::AiRulesArgs;

/// Embedded content from ai-rules.md
const AI_RULES_CONTENT: &str = include_str!("../../ai-rules.md");

#[derive(Debug, Clone, Copy)]
pub enum AiPlatform {
    Cursor,
    Claude,
    Windsurf,
}

impl AiPlatform {
    /// Get the relative file path for this platform
    fn file_path(&self) -> &'static str {
        match self {
            AiPlatform::Cursor => ".cursor/rules/shuttle.mdc",
            AiPlatform::Claude => "CLAUDE.md",
            AiPlatform::Windsurf => ".windsurf/rules/shuttle.md",
        }
    }

    /// Get the display name for this platform
    fn display_name(&self) -> &'static str {
        match self {
            AiPlatform::Cursor => "Cursor",
            AiPlatform::Claude => "Claude Code",
            AiPlatform::Windsurf => "Windsurf",
        }
    }

    /// Get all available platforms
    fn all() -> Vec<AiPlatform> {
        vec![AiPlatform::Cursor, AiPlatform::Claude, AiPlatform::Windsurf]
    }
}

/// Handle the `ai rules` command
pub fn handle_ai_rules(args: &AiRulesArgs, working_directory: &Path) -> Result<()> {
    // Determine platform from args or prompt user
    let platform = if args.cursor {
        AiPlatform::Cursor
    } else if args.claude {
        AiPlatform::Claude
    } else if args.windsurf {
        AiPlatform::Windsurf
    } else {
        // Interactive mode - prompt user to select platform
        select_platform_interactive()?
    };

    // Write the rules file
    write_rules_file(platform, working_directory)?;

    println!(
        "✓ Successfully generated {} rules at: {}",
        platform.display_name(),
        platform.file_path()
    );

    Ok(())
}

/// Prompt user to select a platform interactively
fn select_platform_interactive() -> Result<AiPlatform> {
    let platforms = AiPlatform::all();
    let platform_names: Vec<&str> = platforms.iter().map(|p| p.display_name()).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select AI coding assistant")
        .items(&platform_names)
        .default(0)
        .interact()
        .context("Failed to get platform selection")?;

    Ok(platforms[selection])
}

/// Write the rules file to the appropriate location
fn write_rules_file(platform: AiPlatform, working_directory: &Path) -> Result<()> {
    let file_path = working_directory.join(platform.file_path());

    // Check if file already exists
    if file_path.exists() {
        let overwrite = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "File {} already exists. Overwrite?",
                platform.file_path()
            ))
            .default(false)
            .interact()
            .context("Failed to get confirmation")?;

        if !overwrite {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Create parent directories if they don't exist
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .context(format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write the content
    fs::write(&file_path, AI_RULES_CONTENT)
        .context(format!("Failed to write file: {}", file_path.display()))?;

    Ok(())
}
