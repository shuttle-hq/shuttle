use crate::utils::execute_command;

pub struct GenerateShellParams {
    pub shell: String,
    pub output: Option<String>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct GenerateManpageParams {
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn generate_shell(cwd: String, params: GenerateShellParams) -> Result<String, String> {
    let mut args = vec!["generate".to_string(), "shell".to_string()];

    args.push(params.shell);

    if let Some(output) = params.output {
        args.push("--output".to_string());
        args.push(output);
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

pub async fn generate_manpage(
    cwd: String,
    params: GenerateManpageParams,
) -> Result<String, String> {
    let mut args = vec!["generate".to_string(), "manpage".to_string()];

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
