use axum::{routing::get, Router};
use sync_wrapper::SyncWrapper;

async fn root() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn axum() -> Result<SyncWrapper<Router>, shuttle_service::Error> {
    let router = Router::new().route("/hello", get(root));
    let sync_wrapper = SyncWrapper::new(router);

    Ok(sync_wrapper)
}
