use crate::utils::execute_command;

pub struct AccountParams {
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn account(cwd: String, params: AccountParams) -> Result<String, String> {
    let mut args = vec!["account".to_string()];

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
