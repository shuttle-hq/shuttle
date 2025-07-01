use crate::utils::execute_command;

pub struct ProjectStatusParams {
    pub name: Option<String>,
}

pub struct ProjectListParams {
    pub name: Option<String>,
}

pub async fn project_status(cwd: String, params: ProjectStatusParams) -> Result<String, String> {
    let mut args = vec!["project".to_string(), "status".to_string()];

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
