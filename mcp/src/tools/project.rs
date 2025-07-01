use crate::utils::execute_command;

pub struct ProjectUpdateNameParams {
    pub new_name: String,
    pub name: Option<String>,
}

pub struct ProjectStatusParams {
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct ProjectListParams {
    pub name: Option<String>,
}

pub async fn project_update_name(
    cwd: String,
    params: ProjectUpdateNameParams,
) -> Result<String, String> {
    let mut args = vec![
        "project".to_string(),
        "update".to_string(),
        "name".to_string(),
        params.new_name,
    ];

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    execute_command("shuttle", args, &cwd).await
}

pub async fn project_status(cwd: String, params: ProjectStatusParams) -> Result<String, String> {
    let mut args = vec!["project".to_string(), "status".to_string()];

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

pub async fn project_list(cwd: String, params: ProjectListParams) -> Result<String, String> {
    let mut args = vec!["project".to_string(), "list".to_string()];

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    execute_command("shuttle", args, &cwd).await
}
