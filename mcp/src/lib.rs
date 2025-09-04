use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::ShuttleMcpServer;

mod constants;
mod mcp;
mod tools;
mod utils;

pub async fn run_mcp_server() -> Result<(), anyhow::Error> {
    let service = ShuttleMcpServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
