use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::ShuttleMcpServer;

mod constants;
mod mcp;
mod tools;
mod utils;

pub async fn run_mcp_server() -> Result<(), anyhow::Error> {
    tracing::info!("Starting Shuttle MCP server...");
    let service = ShuttleMcpServer::new().serve(stdio()).await?;
    tracing::info!("Started Shuttle MCP server!");
    service.waiting().await?;
    Ok(())
}
