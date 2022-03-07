use std::str::FromStr;
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use crate::{Status, UNVEIL_PROJECT_HEADER};
use serde::{Deserialize, Serialize};


/// Project names should conform to valid Host segments (or labels)
/// as per [IETF RFC 1123](https://datatracker.ietf.org/doc/html/rfc1123).
/// Initially we'll implement a strict subset of the IETF RFC 1123, concretely:
/// - It does not start or end with `-`.
/// - It does not contain any characters outside of the alphanumeric range, except for `-`.
/// - It is not empty.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct ProjectName(String);

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
            false => Err(ProjectConfigError::InvalidName)
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectConfig {
    pub name: String,
}

impl ProjectConfig {
    pub fn new(name: String) -> Self {
        Self {
            name
        }
    }
}

#[derive(Debug)]
pub enum ProjectConfigError {
    Missing,
    Malformed,
    InvalidName
}

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
            Err(_) => Outcome::Failure((Status::BadRequest, ProjectConfigError::Malformed)),
        }
    }
}

/// Test examples taken from a [Pop-OS project](https://github.com/pop-os/hostname-validator/blob/master/src/lib.rs)
/// and modified to our usecase
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
            assert!(project_name.is_ok(),"{:?} was err", hostname);
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
            assert!(project_name.is_err(),"{:?} was ok", hostname);
        }
    }
}