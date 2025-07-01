use crate::utils::execute_command;

pub struct DeployParams {
    pub secrets: Option<String>,
    pub name: Option<String>,
    pub id: Option<String>,
}

pub struct DeploymentListParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub name: Option<String>,
    pub id: Option<String>,
}

pub struct DeploymentStatusParams {
    pub id: Option<String>,
    pub name: Option<String>,
}

pub async fn deploy(cwd: String, params: DeployParams) -> Result<String, String> {
    let mut args = vec!["deploy".to_string()];

    if let Some(secrets) = params.secrets {
        args.push("--secrets".to_string());
        args.push(secrets);
    }

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

pub async fn deployment_status(
    cwd: String,
    params: DeploymentStatusParams,
) -> Result<String, String> {
    let mut args = vec!["deployment".to_string(), "status".to_string()];

    if let Some(id) = params.id {
        args.push(id);
    }

    if let Some(name) = params.name {
        args.push("--name".to_string());
        args.push(name);
    }

    execute_command("shuttle", args, &cwd).await
}
