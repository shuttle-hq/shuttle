use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::ShuttleMcpServer;

mod constants;
mod mcp;
mod tools;
mod utils;
mod version;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ShuttleMcpServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
