use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::ShuttleMcpServer;

mod mcp;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ShuttleMcpServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
