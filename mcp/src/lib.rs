use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::ShuttleMcpServer;

mod constants;
mod mcp;
mod tools;
mod utils;

pub async fn run_mcp_server() -> Result<(), anyhow::Error> {
    eprintln!("Starting Shuttle MCP server...");
    let service = ShuttleMcpServer::new().serve(stdio()).await?;
    eprintln!("Started Shuttle MCP server!");
    service.waiting().await?;
    Ok(())
}
