mod api;
mod args;
mod error;
mod user;

pub use args::Args;
use tracing::info;

use crate::api::serve;

pub async fn start(args: Args, db: sqlx::Pool<sqlx::Sqlite>) {
    let router = api::ApiBuilder::new().with_sqlite_pool(db).into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, &args.address).await;
}
