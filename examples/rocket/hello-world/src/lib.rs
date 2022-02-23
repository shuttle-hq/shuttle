#[macro_use] extern crate rocket;

use rocket::{Rocket, Build};

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Default)]
struct App;

impl service::Service for App {
    fn setup_rocket(&self, rocket: Rocket<Build>) -> Rocket<Build> {
        rocket.mount("/hello", routes![index])
    }
}

service::declare_service!(App, App::default);
