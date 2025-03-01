use anyhow::{bail, Context, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::debug;
use semver::Version;

const MIN_BACON_VERSION: &str = "3.8.0";

/// Runs bacon in watch mode, checking version requirements first
pub async fn run_bacon(working_directory: &Path) -> Result<()> {
    // Check version and installation
    let output = Command::new("bacon")
        .arg("--version")
        .output()
        .await
        .map_err(|_| anyhow::anyhow!("bacon is not installed - run 'cargo install bacon' to install"))?;

    if !output.status.success() {
        bail!("Failed to get bacon version");
    }

    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = version_str
        .split_whitespace()
        .nth(1)
        .and_then(|v| Version::parse(v).ok())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse bacon version"))?;

    let min_version = Version::parse(MIN_BACON_VERSION).expect("MIN_BACON_VERSION to be valid semver");
    if version < min_version {
        bail!("bacon version {MIN_BACON_VERSION} or higher is required - run 'cargo install bacon' to upgrade");
    }

    debug!("Starting bacon in watch mode...");
    
    // Run bacon with watch mode
    let status = Command::new("bacon")
        .current_dir(working_directory)
        .args(["--job", "run"])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context("failed to start bacon")?
        .wait()
        .await
        .context("failed to wait for bacon process")?;

    if !status.success() {
        bail!("bacon exited with status code: {}", status);
    }

    Ok(())
} 