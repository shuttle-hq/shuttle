use anyhow::{anyhow, Context};
use rand::Rng;
use rocket::form::validate::Contains;
use rocket::http::Status;
use rocket::outcome::try_outcome;
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use rocket::State;
use serde::{Deserialize, Serialize};

use shuttle_common::project::ProjectName;
use shuttle_common::DeploymentApiError;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize, Deserialize, Responder)]
#[serde(transparent)]
pub struct ApiKey(String);

impl ApiKey {
    /// Parses an authorization header string into an ApiKey
    pub fn from_authorization_header<S: AsRef<str>>(header: S) -> Result<Self, AuthorizationError> {
        let s = header.as_ref();
        let parts: Vec<&str> = s.split(' ').collect();
        if parts.len() != 2 {
            return Err(AuthorizationError::Malformed(()));
        }
        // unwrap ok because of explicit check above
        let key = *parts.get(1).unwrap();
        // comes in base64 encoded
        let decoded_bytes = base64::decode(key).map_err(|_| AuthorizationError::Malformed(()))?;
        let mut decoded_string =
            String::from_utf8(decoded_bytes).map_err(|_| AuthorizationError::Malformed(()))?;
        // remove colon at the end
        decoded_string.pop();
        Ok(ApiKey(decoded_string))
    }

    pub fn new_random() -> Self {
        Self(
            rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(16)
                .map(char::from)
                .collect::<String>(),
        )
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
    #[response(status = 500)]
    Internal(()),
    #[response(status = 404)]
    NotFound(()),
}

impl Display for AuthorizationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorizationError::Missing(_) => write!(f, "API key is missing"),
            AuthorizationError::Malformed(_) => write!(f, "API key is malformed"),
            AuthorizationError::Unauthorized(_) => write!(f, "API key is unauthorized"),
            AuthorizationError::AlreadyExists(_) => write!(f, "username already exists"),
            AuthorizationError::Internal(_) => write!(f, "internal server error"),
            AuthorizationError::NotFound(_) => write!(f, "required resource was not found"),
        }
    }
}

impl Error for AuthorizationError {}

/// A wrapper for a Rocket guard that verifies an API key is associated with a
/// valid user.
///
/// The `FromRequest` impl consumes the API key and verifies it is valid for the
/// a user. Generally you want to use [`ScopedUser`] instead to ensure the request
/// is valid against the user's owned resources.
#[derive(Clone, Deserialize, Serialize, Debug)]
pub(crate) struct User {
    pub(crate) name: String,
    pub(crate) projects: Vec<ProjectName>,
}

#[async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = AuthorizationError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if let Some(authorization) = request.headers().get_one("Authorization") {
            match ApiKey::from_authorization_header(authorization) {
                Ok(api_key) => {
                    let authorizer: &'r State<UserDirectory> =
                        try_outcome!(request.guard().await.map_failure(|(status, ())| {
                            (status, AuthorizationError::Internal(()))
                        }));
                    if let Some(user) = authorizer.user_for_api_key(&api_key) {
                        Outcome::Success(user)
                    } else {
                        Outcome::Failure((
                            Status::Unauthorized,
                            AuthorizationError::Unauthorized(()),
                        ))
                    }
                }
                Err(err) => Outcome::Failure((Status::Unauthorized, err)),
            }
        } else {
            Outcome::Failure((Status::Unauthorized, AuthorizationError::Malformed(())))
        }
    }
}

/// A wrapper for a Rocket guard that validates a user's API key *and*
/// scopes the request to a project they own.
///
/// It is guaranteed that [`ScopedUser::scope`] exists and is owned
/// by [`ScopedUser::name`].
pub(crate) struct ScopedUser {
    #[allow(dead_code)]
    user: User,
    scope: ProjectName,
}

impl ScopedUser {
    #[allow(dead_code)]
    pub(crate) fn name(&self) -> &str {
        &self.user.name
    }

    pub(crate) fn scope(&self) -> &ProjectName {
        &self.scope
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for ScopedUser {
    type Error = AuthorizationError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let user = try_outcome!(User::from_request(request).await);
        let route = request
            .route()
            .expect("`User` can only be used in requests");
        if route.uri.base().starts_with("/projects") {
            match request.param::<ProjectName>(0) {
                Some(Ok(scope)) => {
                    if user.projects.contains(&scope) {
                        Outcome::Success(Self { user, scope })
                    } else {
                        Outcome::Failure((
                            Status::Unauthorized,
                            AuthorizationError::Unauthorized(()),
                        ))
                    }
                }
                Some(Err(_)) => {
                    Outcome::Failure((Status::NotFound, AuthorizationError::NotFound(())))
                }
                None => {
                    Outcome::Failure((Status::Unauthorized, AuthorizationError::Unauthorized(())))
                }
            }
        } else {
            panic!("`ScopedUser` can only be used in routes with a /projects/<project_name> scope")
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserDirectory {
    users: RwLock<HashMap<ApiKey, User>>,
}

impl UserDirectory {
    /// Creates a project if it does not already exist
    /// - first there is a check to see if this project exists globally, if yes
    /// will return an error since the project already exists
    /// - if not, will create the project for the user
    /// Finally saves `users` state to `users.toml`.
    pub(crate) fn create_project_if_not_exists(
        &self,
        username: &str,
        project_name: &ProjectName,
    ) -> Result<(), DeploymentApiError> {
        {
            let mut users = self.users.write().unwrap();

            let project_for_name = users
                .values()
                .flat_map(|users| &users.projects)
                .find(|project| project == &project_name);

            if project_for_name.is_some() {
                return Err(DeploymentApiError::ProjectAlreadyExists(format!(
                    "project with name `{}` already exists",
                    project_name
                )));
            }

            // at this point we know that the user does not have this project
            // and that another user does not have it
            let user = users
                .values_mut()
                .find(|u| u.name == username)
                .ok_or_else(|| {
                    DeploymentApiError::Internal(
                    "there was an issue getting the user credentials while validating the project"
                        .to_string(),
                )
                })?;

            user.projects.push(project_name.clone());
        }
        self.save();

        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn authorize(&self, key: &ApiKey, project_name: &ProjectName) -> Option<User> {
        let user = self.user_for_api_key(key)?;
        if user.projects.contains(project_name) {
            Some(user)
        } else {
            None
        }
    }

    /// Find user by username and return it's API Key.
    /// if user does not exist create it and update `users` state to `users.toml`.
    /// Finally return user's API Key.
    pub(crate) fn get_or_create(&self, username: String) -> Result<ApiKey, AuthorizationError> {
        let api_key = {
            let mut users = self.users.write().unwrap();

            if let Some((api_key, _)) = users.iter().find(|(_, user)| user.name == username) {
                api_key.clone()
            } else {
                let api_key = ApiKey::new_random();

                let user = User {
                    name: username,
                    projects: vec![],
                };

                users.insert(api_key.clone(), user);

                api_key
            }
        };

        self.save();

        Ok(api_key)
    }

    /// Overwrites users.toml with the latest users' field data
    fn save(&self) {
        // Save the config
        let mut users_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(Self::users_toml_file_path())
            .unwrap();

        let users = self.users.read().unwrap();

        write!(users_file, "{}", toml::to_string_pretty(&*users).unwrap())
            .expect("could not write contents to users.toml");
    }

    fn user_for_api_key(&self, api_key: &ApiKey) -> Option<User> {
        self.users.read().unwrap().get(api_key).cloned()
    }

    pub(crate) fn from_user_file() -> Result<Self, anyhow::Error> {
        let file_path = Self::users_toml_file_path();
        let file_contents: String = std::fs::read_to_string(&file_path).context(anyhow!(
            "this should blow up if the users.toml file is not present at {:?}",
            &file_path
        ))?;
        let users = toml::from_str(&file_contents)
            .context("this should blow up if the users.toml file is unparseable")?;
        let directory = Self {
            users: RwLock::new(users),
        };

        log::debug!("initialising user directory: {:#?}", &directory);

        Ok(directory)
    }

    fn users_toml_file_path() -> PathBuf {
        match std::env::var("SHUTTLE_USERS_TOML") {
            Ok(val) => val.into(),
            Err(_) => {
                log::debug!("could not find environment variable `SHUTTLE_USERS_TOML`, defaulting to MANIFEST_DIR");
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
        let api_key = ApiKey::from_authorization_header("Basic bXlfYXBpX2tleTo=").unwrap();
        assert_eq!(api_key, ApiKey("my_api_key".to_string()))
    }
}
