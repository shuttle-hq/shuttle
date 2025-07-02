use std::future::Future;

use reqwest::header::{HeaderMap, ORIGIN, USER_AGENT};
use tokio::process::Command;

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
    // Placeholder for running logic before/after every tool
    tool.await
}

pub fn build_client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ORIGIN, "Shuttle MCP".parse().unwrap());
    headers.insert(USER_AGENT, "Shuttle MCP".parse().unwrap());

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))
}
