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

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectCreateArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "The name of the project to create")]
    name: String,
}

pub async fn project_create(params: ProjectCreateArgs) -> Result<String, String> {
    let args = vec![
        "project".to_string(),
        "create".to_string(),
        "--name".to_string(),
        params.name,
    ];

    execute_command("shuttle", args, &params.cwd).await
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectLinkArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "The ID of the project to link")]
    project_id: String,
}

pub async fn project_link(params: ProjectLinkArgs) -> Result<String, String> {
    let args = vec![
        "project".to_string(),
        "link".to_string(),
        "--id".to_string(),
        params.project_id,
    ];

    execute_command("shuttle", args, &params.cwd).await
}
