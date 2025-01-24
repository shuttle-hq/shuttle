// This service cannot be hosted on shuttle since it is missing the runtime the shuttle main macro would have added!!!
async fn axum() -> shuttle_axum::ShuttleAxum {
    let router = axum::Router::new();

    Ok(router.into())
}
