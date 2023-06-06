mod api;
mod args;
mod dal;
mod error;
mod secrets;
mod user;

use std::time::Duration;

use args::StartArgs;
use sqlx::migrate::Migrator;
use thiserror::Error;
use tracing::info;

use crate::api::serve;
pub use api::ApiBuilder;
pub use args::{Args, Commands, InitArgs};
pub use dal::Sqlite;

pub const COOKIE_EXPIRATION: Duration = Duration::from_secs(60 * 60 * 24); // One day

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn start(sqlite: Sqlite, args: StartArgs) {
    let router = api::ApiBuilder::new()
        .with_sqlite(sqlite)
        .with_sessions()
        .into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, args.address).await;
}
