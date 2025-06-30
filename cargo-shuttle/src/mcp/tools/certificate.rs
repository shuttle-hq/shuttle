use crate::mcp::utils::execute_command;

pub struct CertificateAddParams {
    pub domain: String,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct CertificateListParams {
    pub raw: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub struct CertificateDeleteParams {
    pub domain: String,
    pub yes: Option<bool>,
    pub name: Option<String>,
    pub offline: Option<bool>,
    pub debug: Option<bool>,
    pub working_directory: Option<String>,
}

pub async fn certificate_add(cwd: String, params: CertificateAddParams) -> Result<String, String> {
    let mut args = vec!["certificate".to_string(), "add".to_string()];

    args.push(params.domain);

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

pub async fn certificate_list(
    cwd: String,
    params: CertificateListParams,
) -> Result<String, String> {
    let mut args = vec!["certificate".to_string(), "list".to_string()];

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

pub async fn certificate_delete(
    cwd: String,
    params: CertificateDeleteParams,
) -> Result<String, String> {
    let mut args = vec!["certificate".to_string(), "delete".to_string()];

    args.push(params.domain);

    if params.yes.unwrap_or(false) {
        args.push("--yes".to_string());
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
