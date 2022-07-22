use actix_web::{get, web, App, HttpServer, Responder};
use sync_wrapper::SyncWrapper;

#[get("/hello")]
async fn hello_world(name: web::Path<String>) -> impl Responder {
    format!("Hello, world!")
}

#[shuttle_service::main]
async fn actix() -> shuttle_service::ShuttleActix {
    let router = App::new().service(hello_world);
    let sync_wrapper = SyncWrapper::new(router);

    Ok(sync_wrapper)
}
