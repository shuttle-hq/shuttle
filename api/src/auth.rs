use lazy_static::lazy_static;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;
use rand::Rng;
use rocket::form::validate::Contains;
use lib::DeploymentApiError;

#[derive(Debug, PartialEq, Hash, Eq, Deserialize, Serialize, Responder)]
pub struct ApiKey(String);

/// Parses an authorization header string into an ApiKey
impl TryFrom<Option<&str>> for ApiKey {
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
                let key = *parts.get(1).unwrap();
                // comes in base64 encoded
                let decoded_bytes =
                    base64::decode(key).map_err(|_| AuthorizationError::Malformed(()))?;
                let mut decoded_string =
                    String::from_utf8(decoded_bytes).map_err(|_| AuthorizationError::Malformed(()))?;
                // remove colon at the end
                decoded_string.pop();
                Ok(ApiKey(decoded_string))
            }
        }
    }
}

/// A broad class of authorization errors.
/// The empty tuples here are needed by `Responder`.
#[derive(Debug, Responder)]
#[allow(dead_code)]
#[response(content_type = "json")]
pub enum AuthorizationError {
    #[response(status = 400)]
    Missing(()),
    #[response(status = 400)]
    Malformed(()),
    #[response(status = 401)]
    Unauthorized(()),
    #[response(status = 409)]
    AlreadyExists(()),
}

impl Display for AuthorizationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorizationError::Missing(_) => write!(f, "API key is missing"),
            AuthorizationError::Malformed(_) => write!(f, "API key is malformed"),
            AuthorizationError::Unauthorized(_) => write!(f, "API key is unauthorized"),
            AuthorizationError::AlreadyExists(_) => write!(f, "username already exists"),
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
            Err(e) => return Outcome::Failure((Status::BadRequest, e)),
        };
        match USER_DIRECTORY.user_for_api_key(&api_key) {
            None => {
                log::warn!("authorization failure for api key {:?}", &api_key);
                Outcome::Failure((Status::Unauthorized, AuthorizationError::Unauthorized(())))
            }
            Some(user) => Outcome::Success(user),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub(crate) struct User {
    pub(crate) name: String,
    pub(crate) projects: Vec<String>,
}

lazy_static! {
    pub(crate) static ref USER_DIRECTORY: UserDirectory = UserDirectory::from_user_file();
}

#[derive(Debug)]
pub(crate) struct UserDirectory {
    users: RwLock<HashMap<String, User>>,
}

impl UserDirectory {
    /// Validates if a user owns an existing project, if not:
    /// - first there is a check to see if this project exists globally, if yes
    /// will return an error since the project does not belong to the current user
    /// - if not, will create the project for the user
    /// Finally saves `users` state to `users.toml`.
    pub(crate) fn validate_or_create_project(&self, user: &User, project_name: &String) -> Result<(), DeploymentApiError> {
        if user.projects.contains(project_name) {
            return Ok(());
        }

        let mut users = self.users.write().unwrap();

        let project_for_name = users.values()
            .flat_map(|users| &users.projects)
            .find(|project| project == &project_name);

        if project_for_name.is_some() {
            return Err(DeploymentApiError::ProjectAlreadyExists(
                format!("project with name `{}` already exists", project_name)
            ));
        }

        // at this point we know that the user does not have this project
        // and that another user does not have it
        let user = users.values_mut()
            .find(|u| u.name == user.name)
            .ok_or(DeploymentApiError::Internal(
                "there was an issue getting the user credentials while validating the project".to_string()
            )
            )?;

        user.projects.push(project_name.clone());

        self.save(&*users);

        Ok(())
    }

    /// Creates a new user and returns the user's corresponding API Key.
    /// If the user exists, will error.
    /// Finally saves `users` state to `users.toml`.
    pub(crate) fn create_user(&self, username: String) -> Result<ApiKey, AuthorizationError> {
        let mut users = self.users.write().unwrap();

        for user in users.values() {
            if user.name == username {
                return Err(AuthorizationError::AlreadyExists(()));
            }
        }

        let api_key: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        let user = User {
            name: username,
            projects: vec![],
        };

        users.insert(api_key.clone(), user);

        self.save(&*users);

        Ok(ApiKey(api_key))
    }

    /// Overwrites users.toml with a new `HashMap<String, User>`
    fn save(&self, users: &HashMap<String, User>) {
        // Save the config
        let mut users_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(Self::users_toml_file_path())
            .unwrap();

        write!(users_file, "{}", toml::to_string_pretty(&*users).unwrap())
            .expect("could not write contents to users.toml");
    }

    fn user_for_api_key(&self, api_key: &ApiKey) -> Option<User> {
        self.users.read().unwrap().get(&api_key.0).cloned()
    }

    fn from_user_file() -> Self {
        let file_path = Self::users_toml_file_path();
        let file_contents: String = std::fs::read_to_string(&file_path)
            .expect(&format!("this should blow up if the users.toml file is not present at {:?}", &file_path));
        let users = toml::from_str(&file_contents)
            .expect("this should blow up if the users.toml file is unparseable");
        let directory = Self {
            users: RwLock::new(users),
        };

        log::debug!("initialising user directory: {:#?}", &directory);

        directory
    }

    fn users_toml_file_path() -> PathBuf {
        match std::env::var("UNVEIL_USERS_TOML") {
            Ok(val) => val.into(),
            Err(_) => {
                log::debug!("could not find environment variable `UNVEIL_USERS_TOML`, defaulting to MANIFEST_DIR");
                let manifest_path: PathBuf = env!("CARGO_MANIFEST_DIR").into();
                manifest_path.join("users.toml")
            }
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
