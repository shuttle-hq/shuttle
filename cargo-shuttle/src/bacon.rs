use anyhow::{bail, Result};
use semver::Version;
use std::path::Path;
use tokio::process::Command;
use tracing::debug;

const MIN_BACON_VERSION: &str = "3.8.0";
const BACON_CONFIG: &str = r#"[jobs.shuttle]
command = ["shuttle", "run"]
need_stdout = true
allow_warnings = true
background = false
on_change_strategy = "kill_then_restart"
kill = ["pkill", "-TERM", "-P"]"#;

/// Runs shuttle in watch mode using bacon
pub async fn run_bacon(working_directory: &Path) -> Result<()> {
    check_bacon().await?;
    debug!("Starting shuttle in watch mode using bacon...");

    Command::new("bacon")
        .current_dir(working_directory)
        .args(["-j", "shuttle", "--config-toml", BACON_CONFIG])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?
        .wait()
        .await?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("bacon failed"))?;

    Ok(())
}

async fn check_bacon() -> Result<()> {
    let output = Command::new("bacon")
        .arg("--version")
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute bacon: {}\nPlease ensure bacon is installed ('cargo install bacon') and you have the necessary permissions", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_str = stdout.split_whitespace().nth(1).ok_or_else(|| {
        anyhow::anyhow!("Failed to parse bacon version: unexpected output format")
    })?;

    let version = Version::parse(version_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse bacon version '{}': {}", version_str, e))?;
    let min_version = Version::parse(MIN_BACON_VERSION)?;

    if version < min_version {
        bail!("bacon {MIN_BACON_VERSION} or higher required - current version is {version}. Please upgrade using 'cargo install bacon'");
    }

    Ok(())
}
