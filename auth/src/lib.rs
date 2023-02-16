mod api;
mod args;
mod error;
mod user;

use std::net::SocketAddr;

pub use args::Args;
use tracing::info;

pub async fn start(address: SocketAddr, db_uri: &str) {
    let router = api::ApiBuilder::new().with_sqlite_pool(db_uri).await;

    info!(address=%address, "Binding to and listening at address");

    router.serve(address).await;
}
