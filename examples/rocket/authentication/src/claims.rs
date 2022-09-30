use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation,
};
use lazy_static::lazy_static;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    response::status::Custom,
};
use serde::{Deserialize, Serialize};

const BEARER: &str = "Bearer ";
const AUTHORIZATION: &str = "Authorization";

/// Key used for symmetric token encoding
const SECRET: &str = "secret";

lazy_static! {
    /// Time before token expires (aka exp claim)
    static ref TOKEN_EXPIRATION: Duration = Duration::minutes(5);
}

// Used when decoding a token to `Claims`
#[derive(Debug, PartialEq)]
pub(crate) enum AuthenticationError {
    Missing,
    Decoding(String),
    Expired,
}

// Basic claim object. Only the `exp` claim (field) is required. Consult the `jsonwebtoken` documentation for other claims that can be validated.
// The `name` is a custom claim for this API
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Claims {
    pub(crate) name: String,
    exp: usize,
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
    pub(crate) fn from_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            exp: 0,
        }
    }

    /// Create a `Claims` from a 'Bearer <token>' value
    fn from_authorization(value: &str) -> Result<Self, AuthenticationError> {
        let token = value.strip_prefix(BEARER).map(str::trim);

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

    /// Converts this claims into a token string
    pub(crate) fn into_token(mut self) -> Result<String, Custom<String>> {
        let expiration = Utc::now()
            .checked_add_signed(*TOKEN_EXPIRATION)
            .expect("failed to create an expiration time")
            .timestamp();

        self.exp = expiration as usize;

        // Construct and return JWT using `jsonwebtoken`
        // Consult the `jsonwebtoken` documentation for using other algorithms and asymmetric keys
        let token = encode(
            &Header::default(),
            &self,
            &EncodingKey::from_secret(SECRET.as_ref()),
        )
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use crate::claims::AuthenticationError;

    use super::Claims;

    #[test]
    fn missing_bearer() {
        let claim_err = Claims::from_authorization("no-Bearer-prefix").unwrap_err();

        assert_eq!(claim_err, AuthenticationError::Missing);
    }

    #[test]
    fn to_token_and_back() {
        let claim = Claims::from_name("test runner");
        let token = claim.into_token().unwrap();
        let token = format!("Bearer {token}");

        let claim = Claims::from_authorization(&token).unwrap();

        assert_eq!(claim.name, "test runner");
    }
}
