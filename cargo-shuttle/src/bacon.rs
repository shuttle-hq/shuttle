use anyhow::{bail, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::debug;
use semver::Version;

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
    let version = String::from_utf8_lossy(
        &Command::new("bacon")
            .arg("--version")
            .output()
            .await
            .map_err(|_| anyhow::anyhow!("bacon not found - run 'cargo install bacon'"))?
            .stdout
    );

    Version::parse(version.split_whitespace().nth(1).ok_or_else(|| anyhow::anyhow!("invalid bacon version"))?)?
        .lt(&Version::parse(MIN_BACON_VERSION)?)
        .then(|| bail!("bacon {MIN_BACON_VERSION} or higher required - run 'cargo install bacon'"))
        .unwrap_or(Ok(()))
} 