use std::future::Future;
use std::path::PathBuf;

use reqwest::header::{HeaderMap, ORIGIN, USER_AGENT};
use serde::Deserialize;
use tokio::process::Command;
use tracing::debug;

pub async fn execute_command(
    command: &str,
    args: Vec<String>,
    cwd: &str,
) -> Result<String, String> {
    let child = Command::new(command)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(cwd)
        .spawn()
        .map_err(|e| e.to_string())?;

    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;

    let stdout = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let stderr = String::from_utf8(output.stderr).map_err(|e| e.to_string())?;
    let combined = format!("{stdout}\n{stderr}");

    if output.status.success() {
        Ok(combined)
    } else {
        Err(combined)
    }
}

pub async fn run_tool<F>(tool: F) -> Result<String, String>
where
    F: Future<Output = Result<String, String>>,
{
    debug!("Running tool"); // Instrument span wraps the event to display the tool name
    tool.await
}

pub fn build_client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ORIGIN, "Shuttle MCP".parse().unwrap());
    headers.insert(USER_AGENT, "Shuttle MCP".parse().unwrap());

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build client: {e}"))
}

#[derive(Deserialize)]
struct ShuttleConfig {
    id: String,
}

/// Find the project ID by searching for .shuttle/config.toml in the git root
pub async fn find_project_id(cwd: &str) -> Result<String, String> {
    // Start from the working directory and search upward for .git directory
    let mut current_dir = PathBuf::from(cwd);

    // Canonicalize the path to handle relative paths properly
    current_dir = current_dir
        .canonicalize()
        .map_err(|e| format!("Invalid working directory: {}", e))?;

    let git_root = loop {
        if current_dir.join(".git").exists() {
            break current_dir;
        }

        if let Some(parent) = current_dir.parent() {
            current_dir = parent.to_path_buf();
        } else {
            return Err("No .git directory found".to_string());
        }
    };

    // Look for .shuttle/config.toml in the git root
    let config_path = git_root.join(".shuttle").join("config.toml");

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|_| "No project found".to_string())?;

    // Parse the TOML file
    let config: ShuttleConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse .shuttle/config.toml: {}", e))?;

    Ok(config.id)
}
