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

    #[tool(description = "View build and deployment logs")]
    async fn logs(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(
            description = "Deployment ID to get logs for. Defaults to the current deployment"
        )]
        id: Option<String>,
        #[tool(param)]
        #[schemars(description = "View logs from the most recent deployment")]
        latest: Option<bool>,
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
        let mut args = vec!["logs".to_string()];

        if let Some(id) = id {
            args.push(id);
        }

        if latest.unwrap_or(false) {
            args.push("--latest".to_string());
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

    #[tool(description = "Create a project on Shuttle")]
    async fn project_create(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["project".to_string(), "create".to_string()];

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

    #[tool(description = "Update project config - rename the project")]
    async fn project_update_name(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "New name for the project")]
        new_name: String,
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
        let mut args = vec![
            "project".to_string(),
            "update".to_string(),
            "name".to_string(),
            new_name,
        ];

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

    #[tool(description = "Get the status of this project on Shuttle")]
    async fn project_status(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["project".to_string(), "status".to_string()];

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

    #[tool(description = "List all projects you have access to")]
    async fn project_list(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["project".to_string(), "list".to_string()];

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

    #[tool(description = "Delete a project and all linked data")]
    async fn project_delete(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Skip confirmations and proceed")]
        yes: Option<bool>,
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
        let mut args = vec!["project".to_string(), "delete".to_string()];

        if yes.unwrap_or(false) {
            args.push("--yes".to_string());
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

    #[tool(description = "Link this workspace to a Shuttle project")]
    async fn project_link(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["project".to_string(), "link".to_string()];

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

    // Resource Management Commands
    #[tool(description = "List project resources")]
    async fn resource_list(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Show secrets from resources")]
        show_secrets: Option<bool>,
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
        let mut args = vec!["resource".to_string(), "list".to_string()];

        if show_secrets.unwrap_or(false) {
            args.push("--show-secrets".to_string());
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

    #[tool(description = "Delete a resource")]
    async fn resource_delete(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Type of the resource to delete")]
        resource_type: String,
        #[tool(param)]
        #[schemars(description = "Skip confirmations and proceed")]
        yes: Option<bool>,
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
        let mut args = vec!["resource".to_string(), "delete".to_string()];

        args.push(resource_type);

        if yes.unwrap_or(false) {
            args.push("--yes".to_string());
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

    // SSL Certificate Management Commands
    #[tool(description = "Add SSL certificate for custom domain")]
    async fn certificate_add(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Domain name")]
        domain: String,
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
        let mut args = vec!["certificate".to_string(), "add".to_string()];

        args.push(domain);

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

    #[tool(description = "List project certificates")]
    async fn certificate_list(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["certificate".to_string(), "list".to_string()];

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

    #[tool(description = "Delete SSL certificate")]
    async fn certificate_delete(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Domain name")]
        domain: String,
        #[tool(param)]
        #[schemars(description = "Skip confirmations and proceed")]
        yes: Option<bool>,
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
        let mut args = vec!["certificate".to_string(), "delete".to_string()];

        args.push(domain);

        if yes.unwrap_or(false) {
            args.push("--yes".to_string());
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

    // Account Management Commands
    #[tool(description = "Show account info")]
    async fn account(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["account".to_string()];

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

    // Utility Commands
    #[tool(description = "Generate shell completions")]
    async fn generate_shell(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "The shell to generate completion for")]
        shell: String,
        #[tool(param)]
        #[schemars(description = "Output to a file (stdout by default)")]
        output: Option<String>,
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
        let mut args = vec!["generate".to_string(), "shell".to_string()];

        args.push(shell);

        if let Some(output) = output {
            args.push("--output".to_string());
            args.push(output);
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

    #[tool(description = "Generate man page")]
    async fn generate_manpage(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["generate".to_string(), "manpage".to_string()];

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

    #[tool(description = "Open GitHub issue for feedback")]
    async fn feedback(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
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
        let mut args = vec!["feedback".to_string()];

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

    #[tool(description = "Upgrade CLI binary")]
    async fn upgrade(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Install unreleased version from main branch")]
        preview: Option<bool>,
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
        let mut args = vec!["upgrade".to_string()];

        if preview.unwrap_or(false) {
            args.push("--preview".to_string());
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
