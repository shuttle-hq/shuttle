
#[shuttle_service::main]
async fn tide() -> shuttle_service::ShuttleTide {
    let mut app = tide::new();
    app.with(tide::log::LogMiddleware::new());

    app.at("/hello")
        .get(|_| async { Ok("Hello, world!") });

    Ok(app)
}