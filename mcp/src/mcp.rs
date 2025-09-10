use crate::tools::{deployment::*, docs::*, logs::*, project::*};
use crate::utils::run_tool;
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ServerHandler,
};

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
        run_tool(deploy(args)).await
    }

    #[tool(description = "List the deployments for a service")]
    async fn deployment_list(
        &self,
        Parameters(args): Parameters<DeploymentListArgs>,
    ) -> Result<String, String> {
        run_tool(deployment_list(args)).await
    }

    #[tool(description = "View status of a deployment")]
    async fn deployment_status(
        &self,
        Parameters(args): Parameters<DeploymentStatusArgs>,
    ) -> Result<String, String> {
        run_tool(deployment_status(args)).await
    }

    #[tool(description = "View build and deployment logs")]
    async fn logs(&self, Parameters(args): Parameters<LogsArgs>) -> Result<String, String> {
        run_tool(logs(args)).await
    }

    #[tool(description = "Get the status of this project on Shuttle")]
    async fn project_status(
        &self,
        Parameters(args): Parameters<ProjectStatusArgs>,
    ) -> Result<String, String> {
        run_tool(project_status(args)).await
    }

    #[tool(description = "List all projects you have access to")]
    async fn project_list(
        &self,
        Parameters(args): Parameters<ProjectListArgs>,
    ) -> Result<String, String> {
        run_tool(project_list(args)).await
    }

    #[tool(description = "Search Shuttle documentation")]
    async fn search_docs(
        &self,
        Parameters(args): Parameters<SearchDocsArgs>,
    ) -> Result<String, String> {
        run_tool(search_docs(args)).await
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
