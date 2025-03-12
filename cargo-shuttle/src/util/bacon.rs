use anyhow::{bail, Context, Result};
use semver::Version;
use std::path::Path;
use tokio::process::Command;
use tracing::debug;

const MIN_BACON_VERSION: &str = "3.8.0";

#[cfg(unix)]
const BACON_CONFIG: &str = include_str!("bacon.unix.toml");

#[cfg(windows)]
const BACON_CONFIG: &str = include_str!("bacon.windows.toml");

/// Runs shuttle in watch mode using bacon
pub async fn run_bacon(working_directory: &Path) -> Result<()> {
    check_bacon().await?;
    debug!("Starting 'shuttle run' in watch mode using bacon");

    Command::new("bacon")
        .current_dir(working_directory)
        .args([
            "--headless",
            "-j",
            "shuttle-run",
            "--config-toml",
            BACON_CONFIG,
        ])
        .spawn()?
        .wait()
        .await?
        .success()
        .then_some(())
        .context("bacon process failed")?;

    Ok(())
}

async fn check_bacon() -> Result<()> {
    debug!("Checking bacon version");
    let output = Command::new("bacon")
        .arg("--version")
        .output()
        .await
        .context("Failed to execute bacon.\nPlease ensure bacon is installed ('cargo install bacon') and you have the necessary permissions.")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_str = stdout
        .split_whitespace()
        .nth(1)
        .context("Failed to parse bacon version: unexpected output format")?;

    let version = Version::parse(version_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse bacon version '{}': {}", version_str, e))?;
    let min_version = Version::parse(MIN_BACON_VERSION)?;

    if version < min_version {
        bail!("bacon {MIN_BACON_VERSION} or higher required - current version is {version}. Please upgrade using 'cargo install bacon'.");
    }

    Ok(())
}
