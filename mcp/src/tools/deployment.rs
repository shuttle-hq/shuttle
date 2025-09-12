use crate::utils::execute_command;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeployArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Use this secrets file instead")]
    secrets_file: Option<String>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeploymentListArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Which page to display")]
    page: Option<u32>,
    #[schemars(description = "How many deployments per page to display")]
    limit: Option<u32>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeploymentStatusArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "ID of deployment to get status for")]
    deployment_id: Option<String>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

pub async fn deploy(params: DeployArgs) -> Result<String, String> {
    let mut args = vec!["deploy".to_string()];

    if let Some(secrets) = params.secrets_file {
        args.push("--secrets".to_string());
        args.push(secrets);
    }

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

pub async fn deployment_list(params: DeploymentListArgs) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "list".to_string()];

    if let Some(page) = params.page {
        args.push("--page".to_string());
        args.push(page.to_string());
    }

    if let Some(limit) = params.limit {
        args.push("--limit".to_string());
        args.push(limit.to_string());
    }

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

pub async fn deployment_status(params: DeploymentStatusArgs) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "status".to_string()];

    if let Some(id) = params.deployment_id {
        args.push(id);
    }

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
