use crate::utils::execute_command;

fn limit_to_last_n_lines(text: &str, max_lines: u32) -> String {
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() <= max_lines as usize {
        return text.to_string();
    }

    let start_index = lines.len() - max_lines as usize;
    lines[start_index..].join("\n")
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LogsArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Deployment ID to get logs for. Defaults to the current deployment")]
    deployment_id: Option<String>,
    #[schemars(description = "View logs from the most recent deployment")]
    latest: Option<bool>,
    #[schemars(
        description = "Specify the id of the project. Get the project ID by running the project_list tool or create a new project with project_create if none exists"
    )]
    project_id: String,
    #[schemars(description = "Maximum number of lines to return")]
    lines: Option<u32>,
}

pub async fn logs(params: LogsArgs) -> Result<String, String> {
    let mut args = vec!["logs".to_string()];

    if let Some(id) = params.deployment_id {
        args.push(id);
    }

    if params.latest.unwrap_or(false) {
        args.push("--latest".to_string());
    }

    args.push("--id".to_string());
    args.push(params.project_id);

    let output = execute_command("shuttle", args, &params.cwd).await?;

    // Limit the output to the last N lines (default 50)
    let max_lines = params.lines.unwrap_or(50);
    Ok(limit_to_last_n_lines(&output, max_lines))
}
