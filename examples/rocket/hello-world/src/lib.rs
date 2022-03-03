#[macro_use]
extern crate rocket;

use rocket::{Rocket, Build};

#[macro_use]
extern crate unveil_service;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

fn rocket() -> Rocket<Build> {
    rocket::build().mount("/hello", routes![index])
}

declare_service!(Rocket<Build>, rocket);