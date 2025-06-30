use std::future::Future;

use reqwest::header::{HeaderMap, ORIGIN, USER_AGENT};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::mcp::constants::UPGRADE_SHUTTLE_MCP_COMMAND;
use crate::mcp::version::check_new_version;

pub async fn execute_command(
    command: &str,
    args: Vec<String>,
    cwd: &str,
) -> Result<String, String> {
    let mut child = Command::new(command)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(cwd)
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    let mut stdout_lines_stream = stdout_reader.lines();
    let mut stderr_lines_stream = stderr_reader.lines();

    // Read both stdout and stderr concurrently
    tokio::select! {
        _ = async {
            while let Ok(Some(line)) = stdout_lines_stream.next_line().await {
                stdout_lines.push(line);
            }
        } => {},
        _ = async {
            while let Ok(Some(line)) = stderr_lines_stream.next_line().await {
                stderr_lines.push(line);
            }
        } => {},
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;

    let mut all_output = stdout_lines;
    all_output.extend(stderr_lines);
    let result = all_output.join("\n");

    if status.success() {
        Ok(result)
    } else {
        Err(result)
    }
}

pub async fn run_tool<T, F>(tool: T) -> Result<String, String>
where
    T: FnOnce() -> F,
    F: Future<Output = Result<String, String>>,
{
    let has_new_version = check_new_version().await.map_err(|e| e.to_string())?;

    if has_new_version {
        return Ok(format!(
            "A new version of the MCP server is available. Please upgrade to the latest version by running the following command:\n\n{}",
            UPGRADE_SHUTTLE_MCP_COMMAND
        ));
    }

    tool().await
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
