#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    let router = axum::Router::new();

    Ok(router.into())
}
