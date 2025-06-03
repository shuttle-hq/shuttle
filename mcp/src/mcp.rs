use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

use crate::utils::execute_command;

#[derive(Clone)]
pub struct ShuttleMcpServer;

#[tool(tool_box)]
impl ShuttleMcpServer {
    #[tool(description = "Deploy a project using an image")]
    async fn deploy(&self) -> Result<String, String> {
        execute_command("shuttle", &["deploy"])
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
