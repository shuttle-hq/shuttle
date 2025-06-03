use rmcp::ServiceExt;
use shuttle_api_client::ShuttleApiClient;
use tokio::io::{stdin, stdout};

use crate::{mcp::ShuttleMcpServer, utils::load_api_key};

mod mcp;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api_key = load_api_key()?;
    let client = ShuttleApiClient::new(
        "https://api.shuttle.dev".to_string(),
        Some(api_key),
        None,
        None,
    );

    let server = ShuttleMcpServer::new(client);
    let transport = (stdin(), stdout());

    server.serve(transport).await?;

    Ok(())
}
