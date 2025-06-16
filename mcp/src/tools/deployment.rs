use crate::utils::execute_command;

pub struct DeployParams {
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

pub async fn deploy(cwd: String, params: DeployParams) -> Result<String, String> {
    let mut args = vec!["deploy".to_string()];

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
