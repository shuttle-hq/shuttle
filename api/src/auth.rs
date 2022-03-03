use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use lazy_static::lazy_static;
use rocket::{Request};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Hash, Eq, Deserialize, Serialize)]
pub struct ApiKey(String);

/// Parses an authorization header string into an ApiKey
impl TryFrom<Option<&str>> for ApiKey {
    type Error = AuthorizationError;

    fn try_from(s: Option<&str>) -> Result<Self, Self::Error> {
        match s {
            None => Err(AuthorizationError::Missing),
            Some(s) => {
                let parts: Vec<&str> = s.split(" ").collect();
                if parts.len() != 2 {
                    return Err(AuthorizationError::Malformed);
                }
                // unwrap ok because of explicit check above
                let key = *parts.get(1).unwrap();
                // comes in base64 encoded
                let decoded_bytes = base64::decode(key)
                    .map_err(|_| AuthorizationError::Malformed)?;
                let mut decoded_string = String::from_utf8(decoded_bytes)
                    .map_err(|_| AuthorizationError::Malformed)?;
                // remove colon at the end
                decoded_string.pop();
                Ok(ApiKey(decoded_string))
            }
        }
    }
}

#[derive(Debug)]
pub enum AuthorizationError {
    Missing,
    Invalid,
    Malformed,
    Unauthorized,
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

impl Error for AuthorizationError {}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = AuthorizationError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let api_key = match ApiKey::try_from(req.headers().get_one("Authorization")) {
            Ok(api_key) => api_key,
            Err(e) => return Outcome::Failure((Status::BadRequest, e))
        };
        match USER_DIRECTORY.user_for_api_key(&api_key) {
            None => {
                log::warn!("authorization failure for api key {:?}", &api_key);
                Outcome::Failure((Status::Unauthorized, AuthorizationError::Unauthorized))
            }
            Some(user) => Outcome::Success(user)
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct User {
    name: String,
}

lazy_static! {
    static ref USER_DIRECTORY: UserDirectory = UserDirectory::from_user_file();
}

struct UserDirectory {
    users: HashMap<String, User>,
}

impl UserDirectory {
    fn user_for_api_key(&self, api_key: &ApiKey) -> Option<User> {
        self.users.get(&api_key.0).map(|u| u.clone())
    }

    fn from_user_file() -> Self {
        let manifest_path: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        let file_path = manifest_path.join("users.toml");
        let file_contents: String = std::fs::read_to_string(file_path).expect("this should blow up if the users.toml file is not present");
        Self {
            users: toml::from_str(&file_contents).expect("this should blow up if the users.toml file is unparseable")
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::auth::ApiKey;

    #[test]
    pub fn test_api_key_parsing() {
        let api_key: ApiKey = Some("Basic bXlfYXBpX2tleTo=").try_into().unwrap();
        assert_eq!(api_key, ApiKey("my_api_key".to_string()))
    }
}