use once_cell::sync::OnceCell;
use rustrict::{Censor, Type};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Project names must conform to valid Host segments (or labels)
/// as per [IETF RFC 1123](https://datatracker.ietf.org/doc/html/rfc1123).
/// Initially we'll implement a strict subset of the IETF RFC 1123, concretely:
///
/// - It does not start or end with `-`.
/// - It does not contain any characters outside of the alphanumeric range, except for `-`.
/// - It is not empty.
/// - It is shorter than 64 characters.
///
/// Additionaly, while host segments are technically case-insensitive, the filesystem isn't,
/// so we restrict project names to be lower case. We also restrict the use of profanity,
/// as well as a list of reserved words.
#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "backend", derive(sqlx::Type, Hash))]
#[cfg_attr(feature = "backend", sqlx(transparent))]
pub struct ProjectName(String);

impl ProjectName {
    /// The rules a valid project name must follow.
    pub const RULES: &str = "\
    Project names must:
    1. only contain lowercase alphanumeric characters or dashes `-`.
    2. not start or end with a dash.
    3. not be empty.
    4. be shorter than 64 characters.
    5. not contain any profanities.
    6. not be a reserved word.\
    ";

    pub fn is_valid(label: &str) -> bool {
        fn is_valid_char(byte: u8) -> bool {
            matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-')
        }

        fn is_profanity_free(label: &str) -> bool {
            let (_censored, analysis) = Censor::from_str(label).censor_and_analyze();
            !analysis.is(Type::MODERATE_OR_HIGHER)
        }

        fn is_reserved(label: &str) -> bool {
            static INSTANCE: OnceCell<HashSet<&str>> = OnceCell::new();
            INSTANCE.get_or_init(|| HashSet::from(["shuttleapp", "shuttle"]));

            INSTANCE
                .get()
                .expect("Reserved words not set")
                .contains(label)
        }

        !label.is_empty()
            && label.len() < 64
            && !label.starts_with('-')
            && !label.ends_with('-')
            && !is_reserved(label)
            && label.bytes().all(is_valid_char)
            && is_profanity_free(label)
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

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        s.parse().map_err(DeError::custom)
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ProjectName {
    type Err = ProjectNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_owned();
        match ProjectName::is_valid(&s) {
            true => Ok(ProjectName(s)),
            false => Err(ProjectNameError::InvalidName(s)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProjectNameError {
    InvalidName(String),
}

impl Display for ProjectNameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectNameError::InvalidName(name) => {
                let rules = ProjectName::RULES;
                write!(f, "`{name}` is not a valid project name. {rules}")
            }
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
    fn valid_labels() {
        for label in [
            "50-name",
            "235235",
            "123",
            "kebab-case",
            "lowercase",
            "myassets",
            "dachterrasse",
            "another-valid-project-name",
            "x",
        ] {
            let project_name = ProjectName::from_str(label);
            assert!(project_name.is_ok(), "{:?} was err", label);
        }
    }

    #[test]
    fn invalid_labels() {
        for label in [
            "UPPERCASE",
            "CamelCase",
            "pascalCase",
            "InVaLid",
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
            "test-condom-condom",
            "s________e",
            "snake_case",
            "exactly-16-chars\
            exactly-16-chars\
            exactly-16-chars\
            exactly-16-chars",
            "shuttle",
            "shuttleapp",
            "",
        ] {
            let project_name = ProjectName::from_str(label);
            assert!(project_name.is_err(), "{:?} was ok", label);
        }
    }
}
