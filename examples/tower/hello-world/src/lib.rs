use axum::{routing::{get, IntoMakeService}, Router};

use sync_wrapper::SyncWrapper;

async fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn axum() -> Result<SyncWrapper<IntoMakeService<Router>>, shuttle_service::Error> {
    let router = Router::new().route("/hello", get(hello_world));

    let service = router.into_make_service();

    let sync_wrapper = SyncWrapper::new(service);

    Ok(sync_wrapper)
}
