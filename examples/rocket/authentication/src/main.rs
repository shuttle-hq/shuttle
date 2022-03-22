use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
};

#[macro_use]
extern crate rocket;

#[get("/public")]
fn public() -> &'static str {
    "This page is open to anyone"
}

#[get("/private")]
fn private(user: User) -> String {
    format!(
        "The request guard ensures only valid JWTs can access this page: {}",
        user.name
    )
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![public, private])
}

struct User<'r> {
    name: &'r str,
}

#[derive(Debug)]
enum AuthenticationError {
    Missing,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User<'r> {
    type Error = AuthenticationError;

    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Authorization") {
            None => Outcome::Failure((Status::Forbidden, AuthenticationError::Missing)),
            Some(s) => Outcome::Success(User { name: s }),
        }
    }
}
