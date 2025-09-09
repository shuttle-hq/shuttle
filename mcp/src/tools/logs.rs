use crate::utils::execute_command;

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LogsArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Deployment ID to get logs for. Defaults to the current deployment")]
    deployment_id: Option<String>,
    #[schemars(description = "View logs from the most recent deployment")]
    latest: Option<bool>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

pub async fn logs(params: LogsArgs) -> Result<String, String> {
    let mut args = vec!["logs".to_string()];

    if let Some(id) = params.deployment_id {
        args.push(id);
    }

    if params.latest.unwrap_or(false) {
        args.push("--latest".to_string());
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
