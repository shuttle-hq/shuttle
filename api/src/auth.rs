use std::error::Error;
use std::fmt::{Display, Formatter};
use rocket::{Request};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};

#[derive(Debug, PartialEq)]
pub struct ApiKey(String);


pub(crate) trait AuthSystem: Send + Sync {
    fn authorize(&self, api_key: &ApiKey) -> Result<bool, AuthorizationError>;
}

pub(crate) struct TestAuthSystem;

/// Dummy auth system for local testing
impl AuthSystem for TestAuthSystem {
    fn authorize(&self, _api_key: &ApiKey) -> Result<bool, AuthorizationError> {
        Ok(true)
    }
}

impl ApiKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Parses an authorization header string into an ApiKey
impl TryFrom<Option<&str>> for ApiKey {
    type Error = AuthorizationError;

    fn try_from(s: Option<&str>) -> Result<Self, Self::Error> {
        match s {
            None => Err(AuthorizationError::Missing),
            Some(s) => {
                let parts: Vec<&str> = s.split(" ").collect();
                if parts.len() != 2 {
                    return Err(AuthorizationError::Malformed)
                }
                Ok(ApiKey(parts.get(1).unwrap().to_string()))
            }
        }

    }
}

#[derive(Debug)]
pub enum AuthorizationError {
    Missing,
    Invalid,
    Malformed,
    Unauthorized
}

impl Display for AuthorizationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorizationError::Missing => write!(f, "API key is missing"),
            AuthorizationError::Invalid => write!(f, "API key is invalid"),
            AuthorizationError::Malformed => write!(f, "API key is malformed"),
            AuthorizationError::Unauthorized => write!(f, "API key is unauthorized"),
        }
    }
}

impl Error for AuthorizationError {

}

// Request guards are preferred over fairings for auth
impl<'a, 'r> FromRequest<'a, 'r> for ApiKey {
    type Error = AuthorizationError;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let api_key = request.headers().get_one("Authorization");
        match ApiKey::try_from(api_key) {
            Ok(api_key) => {
                // TODO validate here
                Outcome::Success(api_key)
            },
            // token does not exist
            Err(e) => Outcome::Failure((Status::Unauthorized, e))
        }
    }
}

