mod api;
mod args;
mod error;
mod user;

use std::net::SocketAddr;

pub use args::Args;
use tracing::info;

use crate::api::serve;

pub async fn start(address: SocketAddr, db_uri: &str) {
    let router = api::ApiBuilder::new()
        .with_sqlite_pool(db_uri)
        .await
        .into_router();

    info!(address=%address, "Binding to and listening at address");

    serve(router, address).await;
}
