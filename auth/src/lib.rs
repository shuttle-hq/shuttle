mod api;
mod args;
mod error;
mod user;

use std::net::SocketAddr;

pub use args::{Args, Commands, InitArgs};
use tracing::info;

pub async fn start(db_uri: &str, address: SocketAddr, args: Option<InitArgs>) {
    let api_builder = api::ApiBuilder::new().with_sqlite_pool(db_uri).await;

    let router = if let Some(init_args) = args {
        api_builder.init_db(init_args).await
    } else {
        api_builder
    };

    info!(address=%address, "Binding to and listening at address");

    router.serve(address).await;
}
