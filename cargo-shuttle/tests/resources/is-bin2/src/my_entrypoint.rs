#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    Ok(shuttle_axum::axum::Router::new().into())
}
