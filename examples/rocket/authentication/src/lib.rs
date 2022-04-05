use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::{Build, Rocket};
use serde::{Deserialize, Serialize};

mod claims;

use claims::Claims;

#[macro_use]
extern crate rocket;

#[derive(Serialize)]
struct PublicResponse {
    message: String,
}

#[get("/public")]
fn public() -> Json<PublicResponse> {
    Json(PublicResponse {
        message: "This endpoint is open to anyone".to_string(),
    })
}

#[derive(Serialize)]
struct PrivateResponse {
    message: String,
    user: String,
}

// More details on Rocket request guards can be found here
// https://rocket.rs/v0.5-rc/guide/requests/#request-guards
#[get("/private")]
fn private(user: Claims) -> Json<PrivateResponse> {
    Json(PrivateResponse {
        message: "The `Claims` request guard ensures only valid JWTs can access this endpoint"
            .to_string(),
        user: user.name,
    })
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

/// Tries to authenticate a user. Successful authentications get a JWT
#[post("/login", data = "<login>")]
fn login(login: Json<LoginRequest>) -> Result<Json<LoginResponse>, Custom<String>> {
    // This should be real user validation code, but is left simple for this example
    if login.username != "username" || login.password != "password" {
        return Err(Custom(
            Status::Unauthorized,
            "account was not found".to_string(),
        ));
    }

    let claim = Claims::from_name(&login.username);
    let response = LoginResponse {
        token: claim.to_token()?,
    };

    Ok(Json(response))
}

#[shuttle_service::main]
async fn rocket() -> Result<Rocket<Build>, shuttle_service::Error> {
    let rocket = rocket::build().mount("/", routes![public, private, login]);

    Ok(rocket)
}
