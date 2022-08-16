use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use rocket::request::FromParam;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

use once_cell::sync::OnceCell;

/// Project names should conform to valid Host segments (or labels)
/// as per [IETF RFC 1123](https://datatracker.ietf.org/doc/html/rfc1123).
/// Initially we'll implement a strict subset of the IETF RFC 1123, concretely:
/// - It does not start or end with `-` or `_`.
/// - It does not contain any characters outside of the alphanumeric range, except for `-` or '_'.
/// - It is not empty.
/// - It does not contain profanity.
/// - It is not a reserved word.
///
use censor::Censor;

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct ProjectName(String);

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        s.parse().map_err(DeError::custom)
    }
}

impl<'a> FromParam<'a> for ProjectName {
    type Error = ProjectNameError;

    fn from_param(param: &'a str) -> Result<Self, Self::Error> {
        param.parse()
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ProjectName {
    pub fn is_valid(hostname: &str) -> bool {
        fn is_valid_char(byte: u8) -> bool {
            matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_')
        }

        fn is_profanity_free_and_not_reserved(hostname: &str) -> bool {
            static INSTANCE: OnceCell<HashSet<String>> = OnceCell::new();
            INSTANCE.get_or_init(|| HashSet::from(["Shuttle.rs".to_string()]));

            let censor = Censor::Standard
                + Censor::Sex
                + Censor::Zealous
                + Censor::Custom(INSTANCE.get().expect("Reserved words not set").clone())
                - "hell";
            !censor.check(hostname)
        }

        let separators = ['-', '_'];

        !(hostname.bytes().any(|byte| !is_valid_char(byte))
            || !is_profanity_free_and_not_reserved(hostname)
            || hostname.ends_with(separators)
            || hostname.starts_with(separators)
            || hostname.is_empty())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<String> for ProjectName {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

impl FromStr for ProjectName {
    type Err = ProjectNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match ProjectName::is_valid(s) {
            true => Ok(ProjectName(s.to_string())),
            false => Err(ProjectNameError::InvalidName(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum ProjectNameError {
    InvalidName(String),
}

impl Display for ProjectNameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectNameError::InvalidName(name) => write!(
                f,
                r#"
`{}` is an invalid project name. project name must
1. start and end with alphanumeric characters.
2. only contain characters inside of the alphanumeric range, except for `-`, or `_`.
3. not be empty.,
4. not contain profanity.
5. not be a reserved word."#,
                name
            ),
        }
    }
}

impl Error for ProjectNameError {}

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
            "s________e",
            "snake_case",
            "kebab-case",
            "lowercase",
            "UPPERCASE",
            "CamelCase",
            "pascalCase",
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
            "__dunder_like__",
            "__invalid",
            "invalid__",
            "test-crap-crap",
            "shuttle.rs",
        ] {
            let project_name = ProjectName::from_str(hostname);
            assert!(project_name.is_err(), "{:?} was ok", hostname);
        }
    }
}
