use crate::utils::execute_command;

pub struct UpgradeParams {
    pub preview: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn upgrade(cwd: String, params: UpgradeParams) -> Result<String, String> {
    let mut args = vec!["upgrade".to_string()];

    if params.preview.unwrap_or(false) {
        args.push("--preview".to_string());
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
