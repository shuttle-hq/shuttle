use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

use crate::tools::{deployment::*, docs::*, logs::*, project::*};
use crate::utils::run_tool;

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
        secrets_file: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the id of the project")]
        project_id: Option<String>,
    ) -> Result<String, String> {
        run_tool(deploy(
            cwd,
            DeployParams {
                secrets_file,
                name,
                project_id,
            },
        ))
        .await
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
        #[schemars(description = "Specify the name of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the id of the project")]
        project_id: Option<String>,
    ) -> Result<String, String> {
        run_tool(deployment_list(
            cwd,
            DeploymentListParams {
                page,
                limit,
                name,
                project_id,
            },
        ))
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
        deployment_id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the id of the project")]
        project_id: Option<String>,
    ) -> Result<String, String> {
        run_tool(deployment_status(
            cwd,
            DeploymentStatusParams {
                deployment_id,
                name,
                project_id,
            },
        ))
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
        deployment_id: Option<String>,
        #[tool(param)]
        #[schemars(description = "View logs from the most recent deployment")]
        latest: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the id of the project")]
        project_id: Option<String>,
    ) -> Result<String, String> {
        run_tool(logs(
            cwd,
            LogsParams {
                deployment_id,
                latest,
                name,
                project_id,
            },
        ))
        .await
    }

    #[tool(description = "Get the status of this project on Shuttle")]
    async fn project_status(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the id of the project")]
        project_id: Option<String>,
    ) -> Result<String, String> {
        run_tool(project_status(
            cwd,
            ProjectStatusParams { name, project_id },
        ))
        .await
    }

    #[tool(description = "List all projects you have access to")]
    async fn project_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
    ) -> Result<String, String> {
        run_tool(project_list(cwd, ProjectListParams {})).await
    }

    #[tool(description = "Search Shuttle documentation")]
    async fn search_docs(
        &self,
        #[tool(param)]
        #[schemars(description = "Search query for documentation")]
        query: String,
    ) -> Result<String, String> {
        run_tool(search_docs(query)).await
    }
}

#[tool(tool_box)]
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
