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

pub async fn find_project_id(cwd: &str) -> Result<String, String> {
    let mut current_dir = PathBuf::from(cwd);

    current_dir = current_dir.canonicalize().map_err(|_| {
        "The specified working directory does not exist or is inaccessible".to_string()
    })?;

    loop {
        let config_path = current_dir.join(".shuttle").join("config.toml");

        if config_path.exists() {
            let content = tokio::fs::read_to_string(&config_path).await.map_err(|_| {
                "Unable to read the Shuttle configuration file. Check file permissions".to_string()
            })?;

            let config: ShuttleConfig = toml::from_str(&content).map_err(|_| {
                "The Shuttle configuration file is corrupted or invalid".to_string()
            })?;

            return Ok(config.id);
        }

        if let Some(parent) = current_dir.parent() {
            current_dir = parent.to_path_buf();
        } else {
            return Err("No Shuttle project found. Please create a project first".to_string());
        }
    }
}
