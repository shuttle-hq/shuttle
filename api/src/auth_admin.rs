use crate::auth::AuthorizationError;
use lazy_static::lazy_static;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref SHUTTLE_ADMIN_SECRET: String =
        std::env::var("SHUTTLE_ADMIN_SECRET").expect("SHUTTLE_ADMIN_SECRET env var not found!");
}

#[derive(Debug, PartialEq, Hash, Eq, Deserialize, Serialize, Responder)]
pub struct AdminSecret(String);

/// Parses an authorization header string into an AdminSecret
impl TryFrom<Option<&str>> for AdminSecret {
    type Error = AuthorizationError;

    fn try_from(s: Option<&str>) -> Result<Self, Self::Error> {
        match s {
            None => Err(AuthorizationError::Missing(())),
            Some(s) => {
                let parts: Vec<&str> = s.split(' ').collect();

                if parts.len() != 2 {
                    return Err(AuthorizationError::Malformed(()));
                }
                // unwrap ok because of explicit check above
                let secret = *parts.get(1).unwrap();

                Ok(AdminSecret(secret.to_string()))
            }
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Admin {
    type Error = AuthorizationError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let admin_secret = match AdminSecret::try_from(req.headers().get_one("Authorization")) {
            Ok(admin_secret) => admin_secret,
            Err(e) => return Outcome::Failure((Status::BadRequest, e)),
        };

        if admin_secret.0 == *SHUTTLE_ADMIN_SECRET {
            Outcome::Success(Admin {})
        } else {
            log::warn!("authorization failure for admin secret {:?}", &admin_secret);

            Outcome::Failure((Status::Unauthorized, AuthorizationError::Unauthorized(())))
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub(crate) struct Admin {}
