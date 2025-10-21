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
    Gemini,
    Codex,
}

impl AiPlatform {
    /// Get the relative file path for this platform
    fn file_path(&self) -> &'static str {
        match self {
            AiPlatform::Cursor => ".cursor/rules/shuttle.mdc",
            AiPlatform::Claude => "CLAUDE.md",
            AiPlatform::Windsurf => ".windsurf/rules/shuttle.md",
            AiPlatform::Gemini => "GEMINI.md",
            AiPlatform::Codex => "AGENTS.md",
        }
    }

    /// Get the display name for this platform
    fn display_name(&self) -> &'static str {
        match self {
            AiPlatform::Cursor => "Cursor",
            AiPlatform::Claude => "Claude Code",
            AiPlatform::Windsurf => "Windsurf",
            AiPlatform::Gemini => "Gemini CLI",
            AiPlatform::Codex => "Codex CLI",
        }
    }

    /// Get all available platforms
    fn all() -> Vec<AiPlatform> {
        vec![
            AiPlatform::Cursor,
            AiPlatform::Claude,
            AiPlatform::Windsurf,
            AiPlatform::Gemini,
            AiPlatform::Codex,
        ]
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
    } else if args.gemini {
        AiPlatform::Gemini
    } else if args.codex {
        AiPlatform::Codex
    } else {
        // Interactive mode - prompt user to select platform
        select_platform_interactive()?
    };

    // Write the rules file
    let file_path = working_directory.join(platform.file_path());
    let file_existed = file_path.exists();
    let should_append = matches!(platform, AiPlatform::Claude | AiPlatform::Gemini | AiPlatform::Codex) && file_existed;

    let was_written = write_rules_file(platform, working_directory)?;

    if was_written {
        let action = if should_append {
            "appended to"
        } else if file_existed {
            "updated"
        } else {
            "generated"
        };

        println!(
            "✓ Successfully {} {} rules at: {}",
            action,
            platform.display_name(),
            platform.file_path()
        );
    }

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

/// Write the rules file to the appropriate location.
/// Returns Ok(true) if the file was written, Ok(false) if the user aborted.
fn write_rules_file(platform: AiPlatform, working_directory: &Path) -> Result<bool> {
    let file_path = working_directory.join(platform.file_path());

    // For top-level markdown platforms (Claude, Gemini, Codex), append to existing file instead of overwriting
    let should_append = matches!(platform, AiPlatform::Claude | AiPlatform::Gemini | AiPlatform::Codex) && file_path.exists();

    // Check if file already exists
    if file_path.exists() {
        let action = if should_append { "append to" } else { "overwrite" };
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "File {} already exists. {} it?",
                platform.file_path(),
                action.chars().next().unwrap().to_uppercase().to_string() + &action[1..]
            ))
            .default(false)
            .interact()
            .context("Failed to get confirmation")?;

        if !confirm {
            println!("Aborted.");
            return Ok(false);
        }
    }

    // Create parent directories if they don't exist
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .context(format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write or append the content
    if should_append {
        // For top-level markdown files (Claude, Gemini, Codex), append the AI rules to existing file
        let existing_content = fs::read_to_string(&file_path)
            .context(format!("Failed to read existing file: {}", file_path.display()))?;

        // Add separator and append new content
        let combined_content = format!("{}\n\n{}", existing_content.trim_end(), AI_RULES_CONTENT);

        fs::write(&file_path, combined_content)
            .context(format!("Failed to append to file: {}", file_path.display()))?;
    } else {
        // For other platforms or new files, write/overwrite the content
        fs::write(&file_path, AI_RULES_CONTENT)
            .context(format!("Failed to write file: {}", file_path.display()))?;
    }

    Ok(true)
}
