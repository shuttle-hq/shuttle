use crate::mcp::utils::execute_command;

pub struct DeployParams {
    pub image: Option<String>,
    pub allow_dirty: Option<bool>,
    pub output_archive: Option<String>,
    pub no_follow: Option<bool>,
    pub raw: Option<bool>,
    pub secrets: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
    pub name: Option<String>,
}

pub struct DeploymentListParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub raw: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct DeploymentStatusParams {
    pub id: Option<String>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct DeploymentRedeployParams {
    pub id: Option<String>,
    pub no_follow: Option<bool>,
    pub raw: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct DeploymentStopParams {
    pub no_follow: Option<bool>,
    pub raw: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn deploy(cwd: String, params: DeployParams) -> Result<String, String> {
    let mut args = vec!["deploy".to_string()];

    if let Some(image) = params.image {
        args.push("--image".to_string());
        args.push(image);
    }

    if params.allow_dirty.unwrap_or(false) {
        args.push("--allow-dirty".to_string());
    }

    if let Some(output_archive) = params.output_archive {
        args.push("--output-archive".to_string());
        args.push(output_archive);
    }

    if params.no_follow.unwrap_or(false) {
        args.push("--no-follow".to_string());
    }

    if params.raw.unwrap_or(false) {
        args.push("--raw".to_string());
    }

    if let Some(secrets) = params.secrets {
        args.push("--secrets".to_string());
        args.push(secrets);
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

pub async fn deployment_list(cwd: String, params: DeploymentListParams) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "list".to_string()];

    if let Some(page) = params.page {
        args.push("--page".to_string());
        args.push(page.to_string());
    }

    if let Some(limit) = params.limit {
        args.push("--limit".to_string());
        args.push(limit.to_string());
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

pub async fn deployment_status(
    cwd: String,
    params: DeploymentStatusParams,
) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "status".to_string()];

    if let Some(id) = params.id {
        args.push(id);
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

pub async fn deployment_redeploy(
    cwd: String,
    params: DeploymentRedeployParams,
) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "redeploy".to_string()];

    if let Some(id) = params.id {
        args.push(id);
    }

    if params.no_follow.unwrap_or(false) {
        args.push("--no-follow".to_string());
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

pub async fn deployment_stop(cwd: String, params: DeploymentStopParams) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "stop".to_string()];

    if params.no_follow.unwrap_or(false) {
        args.push("--no-follow".to_string());
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
