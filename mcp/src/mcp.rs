use crate::tools::{deployment::*, docs::*, logs::*, project::*};

use crate::utils::run_tool;
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

#[derive(Clone)]
pub struct ShuttleMcpServer;

#[tool(tool_box)]
impl ShuttleMcpServer {
    #[tool(description = "Deploy a project")]
    async fn deploy(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Use this secrets file instead")]
        secrets: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async { deploy(cwd, DeployParams { secrets, name }).await }).await
    }

    #[tool(description = "List the deployments for a service")]
    async fn deployment_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Which page to display")]
        page: Option<u32>,
        #[tool(param)]
        #[schemars(description = "How many deployments per page to display")]
        limit: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            deployment_list(cwd, DeploymentListParams { page, limit, name }).await
        })
        .await
    }

    #[tool(description = "View status of a deployment")]
    async fn deployment_status(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "ID of deployment to get status for")]
        id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async { deployment_status(cwd, DeploymentStatusParams { id, name }).await })
            .await
    }

    #[tool(description = "View build and deployment logs")]
    async fn logs(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
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
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async { logs(cwd, LogsParams { id, latest, name }).await }).await
    }

    #[tool(description = "Get the status of this project on Shuttle")]
    async fn project_status(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async { project_status(cwd, ProjectStatusParams { name }).await }).await
    }

    #[tool(description = "List all projects you have access to")]
    async fn project_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async { project_list(cwd, ProjectListParams { name }).await }).await
    }

    #[tool(description = "Search Shuttle documentation")]
    async fn search_docs(
        &self,
        #[tool(param)]
        #[schemars(description = "Search query for documentation")]
        query: String,
    ) -> Result<String, String> {
        run_tool(|| async { search_docs(query).await }).await
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
