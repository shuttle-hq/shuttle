#[shuttle_service::main]
async fn axum() -> shuttle_service::ShuttleAxum {
    let router = axum::Router::new();

    Ok(router)
}
