use sync_wrapper::SyncWrapper;

#[shuttle_service::main]
async fn tide() -> Result<SyncWrapper<tide::Server<()>>, shuttle_service::Error> {
    let mut app = tide::new();
    app.with(tide::log::LogMiddleware::new());

    app.at("/hello")
        .get(|_| async { Ok("Hello, world!") });

    let sync_wrapper = SyncWrapper::new(app);
    Ok(sync_wrapper)
}
