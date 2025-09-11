use crate::utils::execute_command;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectStatusArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

pub async fn project_status(params: ProjectStatusArgs) -> Result<String, String> {
    let mut args = vec!["project".to_string(), "status".to_string()];

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    if let Some(id) = params.project_id {
        args.push("--id".to_string());
        args.push(id);
    }

    execute_command("shuttle", args, &params.cwd).await
}

pub async fn project_list() -> Result<String, String> {
    let args = vec!["project".to_string(), "list".to_string()];

    execute_command("shuttle", args, ".").await
}
