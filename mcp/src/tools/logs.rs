use crate::utils::execute_command;

pub struct LogsParams {
    pub deployment_id: Option<String>,
    pub latest: Option<bool>,
    pub name: Option<String>,
    pub project_id: Option<String>,
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

    execute_command("shuttle", args, &cwd).await
}
