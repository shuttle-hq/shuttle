#[macro_use] extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

impl service::Service for rocket::Rocket<rocket::Phase::Build> {
    fn start(&self) ->&'static str {
        "sup"
    }
}

service::declare_service!(rocket::Rocket<rocket::Phase::Build>, rocket::build);
