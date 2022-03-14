#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};

#[macro_use]
extern crate shuttle_service;

#[get("/")]
#[allow(unused)]
fn index() -> &'static str {
    "Hello, world!"
}

fn rocket() -> Rocket<Build> {
    panic!()
}

declare_service!(Rocket<Build>, rocket);
