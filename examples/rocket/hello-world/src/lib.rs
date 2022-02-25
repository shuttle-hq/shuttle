#[macro_use] extern crate rocket;

use unveil_service::{Deployment, Service, declare_service};

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Default)]
struct App;

impl Service for App {
    fn deploy(&self) -> Deployment {
        rocket::build().mount("/hello", routes![index]).into()
    }
}

declare_service!(App, App::default);
