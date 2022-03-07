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
        panic!()
    }
}

declare_service!(App, App::default);
