use poem::{get, handler, Route};

#[handler]
fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn main() -> shuttle_service::ShuttlePoem<impl poem::Endpoint> {
    let app = Route::new().at("/hello", get(hello_world));

    Ok(app)
}
