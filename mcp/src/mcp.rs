use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

use crate::utils::execute_command;

#[derive(Clone)]
pub struct ShuttleMcpServer;

#[tool(tool_box)]
impl ShuttleMcpServer {
    #[tool(description = "Deploy a project")]
    async fn deploy(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Allow deployment with uncommitted files")]
        allow_dirty: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Output the deployment archive to a file instead of sending a deployment request"
        )]
        output_archive: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Use this secrets file instead")]
        secrets: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec!["deploy".to_string()];

        if allow_dirty.unwrap_or(false) {
            args.push("--allow-dirty".to_string());
        }

        if let Some(output_archive) = output_archive {
            args.push("--output-archive".to_string());
            args.push(output_archive);
        }

        if no_follow.unwrap_or(false) {
            args.push("--no-follow".to_string());
        }

        if raw.unwrap_or(false) {
            args.push("--raw".to_string());
        }

        if let Some(secrets) = secrets {
            args.push("--secrets".to_string());
            args.push(secrets);
        }

        if offline.unwrap_or(false) {
            args.push("--offline".to_string());
        }

        if debug.unwrap_or(false) {
            args.push("--debug".to_string());
        }

        if let Some(working_directory) = working_directory {
            args.push("--working-directory".to_string());
            args.push(working_directory);
        }

        if let Some(name) = name {
            args.push("--name".to_string());
            args.push(name);
        }

        execute_command("shuttle", args, &cwd).await
    }

    #[tool(description = "List the deployments for a service")]
    async fn deployment_list(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Which page to display")]
        page: Option<u32>,
        #[tool(param)]
        #[schemars(description = "How many deployments per page to display")]
        limit: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Output tables without borders")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec!["deployment".to_string(), "list".to_string()];

        if let Some(page) = page {
            args.push("--page".to_string());
            args.push(page.to_string());
        }

        if let Some(limit) = limit {
            args.push("--limit".to_string());
            args.push(limit.to_string());
        }

        if raw.unwrap_or(false) {
            args.push("--raw".to_string());
        }

        if offline.unwrap_or(false) {
            args.push("--offline".to_string());
        }

        if debug.unwrap_or(false) {
            args.push("--debug".to_string());
        }

        if let Some(working_directory) = working_directory {
            args.push("--working-directory".to_string());
            args.push(working_directory);
        }

        if let Some(name) = name {
            args.push("--name".to_string());
            args.push(name);
        }

        execute_command("shuttle", args, &cwd).await
    }

    #[tool(description = "View status of a deployment")]
    async fn deployment_status(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "ID of deployment to get status for")]
        id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec!["deployment".to_string(), "status".to_string()];

        if let Some(id) = id {
            args.push(id);
        }

        if offline.unwrap_or(false) {
            args.push("--offline".to_string());
        }

        if debug.unwrap_or(false) {
            args.push("--debug".to_string());
        }

        if let Some(working_directory) = working_directory {
            args.push("--working-directory".to_string());
            args.push(working_directory);
        }

        if let Some(name) = name {
            args.push("--name".to_string());
            args.push(name);
        }

        execute_command("shuttle", args, &cwd).await
    }

    #[tool(description = "Redeploy a previous deployment")]
    async fn deployment_redeploy(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "ID of deployment to redeploy")]
        id: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec!["deployment".to_string(), "redeploy".to_string()];

        if let Some(id) = id {
            args.push(id);
        }

        if no_follow.unwrap_or(false) {
            args.push("--no-follow".to_string());
        }

        if raw.unwrap_or(false) {
            args.push("--raw".to_string());
        }

        if offline.unwrap_or(false) {
            args.push("--offline".to_string());
        }

        if debug.unwrap_or(false) {
            args.push("--debug".to_string());
        }

        if let Some(working_directory) = working_directory {
            args.push("--working-directory".to_string());
            args.push(working_directory);
        }

        if let Some(name) = name {
            args.push("--name".to_string());
            args.push(name);
        }

        execute_command("shuttle", args, &cwd).await
    }

    #[tool(description = "Stop running deployment(s)")]
    async fn deployment_stop(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec!["deployment".to_string(), "stop".to_string()];

        if no_follow.unwrap_or(false) {
            args.push("--no-follow".to_string());
        }

        if raw.unwrap_or(false) {
            args.push("--raw".to_string());
        }

        if offline.unwrap_or(false) {
            args.push("--offline".to_string());
        }

        if debug.unwrap_or(false) {
            args.push("--debug".to_string());
        }

        if let Some(working_directory) = working_directory {
            args.push("--working-directory".to_string());
            args.push(working_directory);
        }

        if let Some(name) = name {
            args.push("--name".to_string());
            args.push(name);
        }

        execute_command("shuttle", args, &cwd).await
    }
}

#[tool(tool_box)]
impl ServerHandler for ShuttleMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Shuttle MCP server providing CLI deployment and project management tools".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
