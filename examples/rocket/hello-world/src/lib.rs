#[macro_use] extern crate rocket;

use rocket::{Rocket, Build};

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

struct MyRocket(Rocket<Build>);

impl MyRocket {
    fn new() -> Self {
        MyRocket(rocket::build().mount("/hello", routes![index]))
    }
}

impl service::Service for MyRocket {
    fn start(&self) ->&'static str {
        "sup"
    }

    fn my_rocket(&self) -> &Rocket<Build> {
        &self.0
    }
}

service::declare_service!(MyRocket, MyRocket::new);
