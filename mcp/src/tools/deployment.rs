use crate::utils::execute_command;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeployArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Use this secrets file instead")]
    secrets_file: Option<String>,
    #[schemars(
        description = "Specify the id of the project. Get the project ID by running the project_list tool or create a new project with project_create if none exists"
    )]
    project_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeploymentListArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Which page to display")]
    page: Option<u32>,
    #[schemars(description = "How many deployments per page to display")]
    limit: Option<u32>,
    #[schemars(
        description = "Specify the id of the project. Get the project ID by running the project_list tool or create a new project with project_create if none exists"
    )]
    project_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeploymentStatusArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "ID of deployment to get status for")]
    deployment_id: Option<String>,
    #[schemars(
        description = "Specify the id of the project. Get the project ID by running the project_list tool or create a new project with project_create if none exists"
    )]
    project_id: String,
}

pub async fn deploy(params: DeployArgs) -> Result<String, String> {
    let mut args = vec!["deploy".to_string()];

    if let Some(secrets) = params.secrets_file {
        args.push("--secrets".to_string());
        args.push(secrets);
    }

    args.push("--id".to_string());
    args.push(params.project_id);

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

    args.push("--id".to_string());
    args.push(params.project_id);

    execute_command("shuttle", args, &params.cwd).await
}

pub async fn deployment_status(params: DeploymentStatusArgs) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "status".to_string()];

    if let Some(id) = params.deployment_id {
        args.push(id);
    }

    args.push("--id".to_string());
    args.push(params.project_id);

    execute_command("shuttle", args, &params.cwd).await
}
