pub fn execute_command(command: &str, args: Vec<String>) -> Result<String, String> {
    let output = std::process::Command::new(command)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let pwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let result = format!("pwd: {}\nstdout: {}\nstderr: {}", pwd, stdout, stderr);

    if output.status.success() {
        Ok(result)
    } else {
        Err(result)
    }
}
