use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ServerHandler,
};

use crate::tools::{deployment::*, docs::*, logs::*, project::*};
use crate::utils::run_tool;

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct DeployArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Use this secrets file instead")]
    secrets_file: Option<String>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct DeploymentListArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Which page to display")]
    page: Option<u32>,
    #[schemars(description = "How many deployments per page to display")]
    limit: Option<u32>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct DeploymentStatusArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "ID of deployment to get status for")]
    deployment_id: Option<String>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct LogsArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Deployment ID to get logs for. Defaults to the current deployment")]
    deployment_id: Option<String>,
    #[schemars(description = "View logs from the most recent deployment (which is not always the running one)")]
    latest: Option<bool>,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
    #[schemars(description = "View the last N log lines")]
    lines: Option<u32>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct ProjectStatusArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
    #[schemars(description = "Specify the name of the project")]
    name: Option<String>,
    #[schemars(description = "Specify the id of the project")]
    project_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct ProjectListArgs {
    #[schemars(description = "Specify the working directory")]
    cwd: String,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct SearchDocsArgs {
    #[schemars(description = "Search query for documentation")]
    query: String,
}

#[derive(Clone)]
pub struct ShuttleMcpServer {
    pub(crate) tool_router: ToolRouter<Self>,
}

impl ShuttleMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ShuttleMcpServer {
    #[tool(description = "Deploy a project")]
    async fn deploy(&self, Parameters(args): Parameters<DeployArgs>) -> Result<String, String> {
        run_tool(deploy(
            args.cwd,
            DeployParams {
                secrets_file: args.secrets_file,
                name: args.name,
                project_id: args.project_id,
            },
        ))
        .await
    }

    #[tool(description = "List the deployments for a service")]
    async fn deployment_list(
        &self,
        Parameters(args): Parameters<DeploymentListArgs>,
    ) -> Result<String, String> {
        run_tool(deployment_list(
            args.cwd,
            DeploymentListParams {
                page: args.page,
                limit: args.limit,
                name: args.name,
                project_id: args.project_id,
            },
        ))
        .await
    }

    #[tool(description = "View status of a deployment")]
    async fn deployment_status(
        &self,
        Parameters(args): Parameters<DeploymentStatusArgs>,
    ) -> Result<String, String> {
        run_tool(deployment_status(
            args.cwd,
            DeploymentStatusParams {
                deployment_id: args.deployment_id,
                name: args.name,
                project_id: args.project_id,
            },
        ))
        .await
    }

    #[tool(description = "View build and deployment logs")]
    async fn logs(&self, Parameters(args): Parameters<LogsArgs>) -> Result<String, String> {
        run_tool(logs(
            args.cwd,
            LogsParams {
                deployment_id: args.deployment_id,
                latest: args.latest,
                name: args.name,
                project_id: args.project_id,
                lines: args.lines,
            },
        ))
        .await
    }

    #[tool(description = "Get the status of this project on Shuttle")]
    async fn project_status(
        &self,
        Parameters(args): Parameters<ProjectStatusArgs>,
    ) -> Result<String, String> {
        run_tool(project_status(
            args.cwd,
            ProjectStatusParams {
                name: args.name,
                project_id: args.project_id,
            },
        ))
        .await
    }

    #[tool(description = "List all projects you have access to")]
    async fn project_list(
        &self,
        Parameters(args): Parameters<ProjectListArgs>,
    ) -> Result<String, String> {
        run_tool(project_list(args.cwd, ProjectListParams {})).await
    }

    #[tool(description = "Search Shuttle documentation")]
    async fn search_docs(
        &self,
        Parameters(args): Parameters<SearchDocsArgs>,
    ) -> Result<String, String> {
        run_tool(search_docs(args.query)).await
    }
}

#[tool_handler]
impl ServerHandler for ShuttleMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Shuttle MCP server providing docs search, CLI deployment and project management tools".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
