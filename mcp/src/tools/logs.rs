use crate::utils::execute_command;

pub struct LogsParams {
    pub deployment_id: Option<String>,
    pub latest: Option<bool>,
    pub name: Option<String>,
    pub project_id: Option<String>,
    pub lines: Option<u32>,
}

fn limit_to_last_n_lines(text: &str, max_lines: u32) -> String {
    let lines: Vec<&str> = text.lines().collect();
    
    if lines.len() <= max_lines as usize {
        return text.to_string();
    }

    let start_index = lines.len() - max_lines as usize;
    lines[start_index..].join("\n")
}

pub async fn logs(cwd: String, params: LogsParams) -> Result<String, String> {
    let mut args = vec!["logs".to_string()];

    if let Some(id) = params.deployment_id {
        args.push(id);
    }

    if params.latest.unwrap_or(false) {
        args.push("--latest".to_string());
    }

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    if let Some(id) = params.project_id {
        args.push("--id".to_string());
        args.push(id);
    }

    let output = execute_command("shuttle", args, &cwd).await?;

    // Limit the output to the last N lines (default 50)
    let max_lines = params.lines.unwrap_or(50);
    Ok(limit_to_last_n_lines(&output, max_lines))
}
