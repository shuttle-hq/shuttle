use crate::utils::execute_command;

pub struct ProjectStatusParams {
    pub name: Option<String>,
    pub id: Option<String>,
}

pub struct ProjectListParams {}

pub async fn project_status(cwd: String, params: ProjectStatusParams) -> Result<String, String> {
    let mut args = vec!["project".to_string(), "status".to_string()];

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    if let Some(id) = params.id {
        args.push("--id".to_string());
        args.push(id);
    }

    execute_command("shuttle", args, &cwd).await
}

pub async fn project_list(cwd: String, _params: ProjectListParams) -> Result<String, String> {
    let args = vec!["project".to_string(), "list".to_string()];

    execute_command("shuttle", args, &cwd).await
}
