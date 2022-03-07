use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use serde::{Deserialize, Serialize};
use serde::de::Error as DeError;
use crate::{Status, UNVEIL_PROJECT_HEADER};


/// Project names should conform to valid Host segments (or labels)
/// as per [IETF RFC 1123](https://datatracker.ietf.org/doc/html/rfc1123).
/// Initially we'll implement a strict subset of the IETF RFC 1123, concretely:
/// - It does not start or end with `-`.
/// - It does not contain any characters outside of the alphanumeric range, except for `-`.
/// - It is not empty.
#[derive(Clone, Serialize, Debug)]
struct ProjectName(String);

fn deserialize_project_name<'de, D>(deserializer: D) -> Result<ProjectName, D::Error>
    where
        D: serde::Deserializer<'de> {
    let s: String = String::deserialize(deserializer)?;

    s.parse().map_err(DeError::custom)
}


impl ProjectName {
    pub fn is_valid(hostname: &str) -> bool {
        fn is_valid_char(byte: u8) -> bool {
            (byte >= b'a' && byte <= b'z')
                || (byte >= b'A' && byte <= b'Z')
                || (byte >= b'0' && byte <= b'9')
                || byte == b'-'
        }

        !(hostname.bytes().any(|byte| !is_valid_char(byte))
            || hostname.ends_with('-')
            || hostname.starts_with('-')
            || hostname.is_empty())
    }
}

impl FromStr for ProjectName {
    type Err = ProjectConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match ProjectName::is_valid(s) {
            true => Ok(ProjectName(s.to_string())),
            false => Err(ProjectConfigError::InvalidName(s.to_string()))
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectConfig {
    #[serde(deserialize_with = "deserialize_project_name")]
    name: ProjectName,
}

impl ProjectConfig {
    pub fn new(name: String) -> Result<Self, ProjectConfigError> {
        Ok(Self {
            name: (&name).parse()?
        })
    }

    pub fn name(&self) -> &String {
        &self.name.0
    }
}

#[derive(Debug)]
pub enum ProjectConfigError {
    Missing,
    Malformed(String),
    InvalidName(String),
}

impl ProjectConfigError {
    fn malformed(msg: &str) -> Self {
        Self::Malformed(msg.to_string())
    }
}

impl Display for ProjectConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectConfigError::Missing => write!(f, "missing"),
            ProjectConfigError::Malformed(msg) => write!(f, "malformed: {}", msg),
            ProjectConfigError::InvalidName(name) => write!(f, r#"
`{}` is an invalid project name. project name must
1. not start or end with `-`.
2. not contain any characters outside of the alphanumeric range, except for `-`.
3. not be empty."#, name),
        }
    }
}

impl Error for ProjectConfigError {}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ProjectConfig {
    type Error = ProjectConfigError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config_string = match req.headers().get_one(UNVEIL_PROJECT_HEADER) {
            None => return Outcome::Failure((Status::BadRequest, ProjectConfigError::Missing)),
            Some(config_string) => config_string,
        };

        match serde_json::from_str::<ProjectConfig>(config_string) {
            Ok(config) => Outcome::Success(config),
            Err(_) => Outcome::Failure((Status::BadRequest, ProjectConfigError::malformed("could not parse project config from json"))),
        }
    }
}

/// Test examples taken from a [Pop-OS project](https://github.com/pop-os/hostname-validator/blob/master/src/lib.rs)
/// and modified to our use case
#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn valid_hostnames() {
        for hostname in [
            "VaLiD-HoStNaMe",
            "50-name",
            "235235",
            "VaLid",
            "123",
        ] {
            let project_name = ProjectName::from_str(hostname);
            assert!(project_name.is_ok(), "{:?} was err", hostname);
        }
    }

    #[test]
    fn invalid_hostnames() {
        for hostname in [
            "-invalid-name",
            "also-invalid-",
            "asdf@fasd",
            "@asdfl",
            "asd f@",
            ".invalid",
            "invalid.name",
            "invalid.name.",
        ] {
            let project_name = ProjectName::from_str(hostname);
            assert!(project_name.is_err(), "{:?} was ok", hostname);
        }
    }
}