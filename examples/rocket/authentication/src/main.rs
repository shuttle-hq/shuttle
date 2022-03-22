use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation,
};
use lazy_static::lazy_static;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    response::status::Custom,
    serde::json::Json,
};
use serde::{Deserialize, Serialize};

#[macro_use]
extern crate rocket;

const BEARER: &str = "Bearer ";
const AUTHORIZATION: &str = "Authorization";
const SECRET: &str = "secret";

lazy_static! {
    static ref TOKEN_EXPIRATION: Duration = Duration::minutes(5);
}

#[get("/public")]
fn public() -> Json<PublicResponse> {
    Json(PublicResponse {
        message: "This endpoint is open to anyone".to_string(),
    })
}

#[get("/private")]
fn private(user: Claims) -> Json<PrivateResponse> {
    Json(PrivateResponse {
        message: "The request guard ensures only valid JWTs can access this endpoint".to_string(),
        user: user.name,
    })
}

/// Tries to authenticate a user. Successful authentications get a JWT token
#[post("/login", data = "<login>")]
fn login(login: Json<LoginRequest>) -> Result<String, Custom<String>> {
    // This should be real user validation code, but is left simple for this example
    if login.username != "username" || login.password != "password" {
        return Err(Custom(
            Status::Unauthorized,
            "account was not found".to_string(),
        ));
    }

    let expiration = Utc::now()
        .checked_add_signed(*TOKEN_EXPIRATION)
        .expect("failed to create an expiration time")
        .timestamp();

    let claims = Claims {
        name: login.username.to_string(),
        exp: expiration as usize,
    };

    // Construct and return JWT using `jsonwebtoken`
    // Consult the `jsonwebtoken` documentation for using other algorithms and asymmetric keys
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET.as_ref()),
    )
    .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    Ok(token)
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![public, private, login])
}

#[derive(Serialize, Deserialize)]
struct Claims {
    name: String,
    exp: usize,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct PublicResponse {
    message: String,
}

#[derive(Serialize)]
struct PrivateResponse {
    message: String,
    user: String,
}

#[derive(Debug)]
enum AuthenticationError {
    Missing,
    Decoding(String),
    Expired,
}

// Rocket specific request guard implementation
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = AuthenticationError;

    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one(AUTHORIZATION) {
            None => Outcome::Failure((Status::Forbidden, AuthenticationError::Missing)),
            Some(value) => match Claims::from_authorization(value) {
                Err(e) => Outcome::Failure((Status::Forbidden, e)),
                Ok(claims) => Outcome::Success(claims),
            },
        }
    }
}

impl Claims {
    fn from_authorization(value: &str) -> Result<Self, AuthenticationError> {
        let token = value.strip_prefix(BEARER);

        if token.is_none() {
            return Err(AuthenticationError::Missing);
        }

        // Safe to unwrap as we just confirmed it is not none
        let token = token.unwrap();

        // Use `jsonwebtoken` to get the claims from a JWT
        // Consult the `jsonwebtoken` documentation for using other algorithms and validations (the default validation just checks the expiration claim)
        let token = decode::<Claims>(
            token,
            &DecodingKey::from_secret(SECRET.as_ref()),
            &Validation::default(),
        )
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthenticationError::Expired,
            _ => AuthenticationError::Decoding(e.to_string()),
        })?;

        Ok(token.claims)
    }
}
