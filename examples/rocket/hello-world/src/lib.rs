#[macro_use] extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Default)]
struct App;

impl service::Service for App {
    fn deploy(&self) -> service::Deployment {
        rocket::build().mount("/hello", routes![index]).into()
    }
}

service::declare_service!(App, App::default);
