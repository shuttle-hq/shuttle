use crate::mcp::utils::execute_command;

pub struct LogsParams {
    pub id: Option<String>,
    pub latest: Option<bool>,
    pub follow: Option<bool>,
    pub raw: Option<bool>,
    pub head: Option<u32>,
    pub tail: Option<u32>,
    pub all: Option<bool>,
    pub all_deployments: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn logs(cwd: String, params: LogsParams) -> Result<String, String> {
    let mut args = vec!["logs".to_string()];

    if let Some(id) = params.id {
        args.push(id);
    }

    if params.latest.unwrap_or(false) {
        args.push("--latest".to_string());
    }

    if params.follow.unwrap_or(false) {
        args.push("--follow".to_string());
    }

    if params.raw.unwrap_or(false) {
        args.push("--raw".to_string());
    }

    if let Some(head) = params.head {
        args.push("--head".to_string());
        args.push(head.to_string());
    }

    if let Some(tail) = params.tail {
        args.push("--tail".to_string());
        args.push(tail.to_string());
    }

    if params.all.unwrap_or(false) {
        args.push("--all".to_string());
    }

    if params.all_deployments.unwrap_or(false) {
        args.push("--all-deployments".to_string());
    }

    if params.offline.unwrap_or(false) {
        args.push("--offline".to_string());
    }

    if params.debug.unwrap_or(false) {
        args.push("--debug".to_string());
    }

    if let Some(working_directory) = params.working_directory {
        args.push("--working-directory".to_string());
        args.push(working_directory);
    }

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    execute_command("shuttle", args, &cwd).await
}
