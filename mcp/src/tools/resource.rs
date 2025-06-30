use crate::utils::execute_command;

pub struct ResourceListParams {
    pub show_secrets: Option<bool>,
    pub raw: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct ResourceDeleteParams {
    pub resource_type: String,
    pub yes: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn resource_list(cwd: String, params: ResourceListParams) -> Result<String, String> {
    let mut args = vec!["resource".to_string(), "list".to_string()];

    if params.show_secrets.unwrap_or(false) {
        args.push("--show-secrets".to_string());
    }

    if params.raw.unwrap_or(false) {
        args.push("--raw".to_string());
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

pub async fn resource_delete(cwd: String, params: ResourceDeleteParams) -> Result<String, String> {
    let mut args = vec!["resource".to_string(), "delete".to_string()];

    args.push(params.resource_type);

    if params.yes.unwrap_or(false) {
        args.push("--yes".to_string());
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
