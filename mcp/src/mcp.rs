use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::models::deployment::{DeploymentRequest, DeploymentRequestImage};

#[derive(Clone)]
pub struct ShuttleMcpServer {
    client: ShuttleApiClient,
}

#[tool(tool_box)]
impl ShuttleMcpServer {
    pub fn new(client: ShuttleApiClient) -> Self {
        Self { client }
    }

    #[tool(description = "Deploy a project using an image")]
    async fn deploy(
        &self,
        #[tool(param)]
        #[schemars(description = "The project name to deploy")]
        project: String,
        #[tool(param)]
        #[schemars(description = "The Docker image to deploy")]
        image: String,
    ) -> Result<String, String> {
        let deployment_req = DeploymentRequest::Image(DeploymentRequestImage {
            image,
            secrets: None,
        });

        match self.client.deploy(&project, deployment_req).await {
            Ok(deployment) => match serde_json::to_string_pretty(&deployment) {
                Ok(json) => Ok(json),
                Err(e) => Err(format!("Failed to serialize deployment: {}", e)),
            },
            Err(e) => Err(format!("Failed to deploy project: {}", e)),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for ShuttleMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "A Shuttle API MCP server that provides deployment functionality".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
