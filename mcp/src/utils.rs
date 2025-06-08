use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub async fn execute_command(command: &str, args: Vec<String>) -> Result<String, String> {
    let mut child = Command::new(command)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
