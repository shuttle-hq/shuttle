use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::ShuttleMcpServer;

mod mcp;
mod utils;
mod version;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: do a check and make sure the latest version is installed for both the shuttle cli
    // And the MCP server

    let has_new_version = version::check_new_version().await?;

    if has_new_version {
        println!(
            "A new version of the MCP server is available. Please upgrade to the latest version."
        );
        std::process::exit(1);
    }

    let service = ShuttleMcpServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
