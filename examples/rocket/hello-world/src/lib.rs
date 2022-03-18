#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};

#[macro_use]
extern crate shuttle_service;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}
async fn wrapper(
    _factory: &mut dyn shuttle_service::Factory,
) -> Result<Rocket<Build>, shuttle_service::Error> {
    rocket().await
}

declare_service!(wrapper);

async fn rocket() -> Result<Rocket<Build>, shuttle_service::Error> {
    let rocket = rocket::build().mount("/hello", routes![index]);

    Ok(rocket)
}
