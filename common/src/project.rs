use once_cell::sync::OnceCell;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use rustrict::{Censor, Type};

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

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ProjectName {
    /// Project names should conform to valid Host segments (or labels)
    /// as per [IETF RFC 1123](https://datatracker.ietf.org/doc/html/rfc1123).
    /// We implement a strict subset of the IETF RFC 1123, concretely:
    /// - It does not start or end with `-`.
    /// - It does not contain any characters outside of the alphanumeric range, except for `-`.
    /// - It is between 1 and 63 characters in length.
    ///
    /// Additionaly, while host segments are technically case-insensitive, the filesystem isn't,
    /// so we restrict project names to be lower case. We also restrict the use of profanity,
    /// as well as a list of reserved words.
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

        // each label in a hostname can be between 1 and 63 chars
        let is_invalid_length = label.len() > 63;

        !(label.bytes().any(|byte| !is_valid_char(byte))
            || is_reserved(label)
            || !is_profanity_free(label)
            || label.ends_with('-')
            || label.starts_with('-')
            || label.is_empty()
            || is_invalid_length)
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
3. be lowercase.
4. be between 1 and 63 characters in length.
5. not contain profanity.
6. not be a reserved word.
"#,
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
    fn valid_labels() {
        for label in [
            "50-name",
            "235235",
            "123",
            "s------e",
            "kebab-case",
            "lowercase",
            "myassets",
            "dachterrasse",
            "thisoneislongbutvalidthisoneislongbutvalidthisoneislongbutvalid",
        ] {
            let project_name = ProjectName::from_str(label);
            assert!(project_name.is_ok(), "{:?} was err", label);
        }
    }

    #[test]
    fn invalid_labels() {
        for label in [
            "inVaLiD-HoStNaMeinVaLiD",
            "CamelCase",
            "snake_case",
            "pascalCase",
            "inVaLid",
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
            "shuttle.rs",
            "thisoneislongandinvalidthisoneislongandinvalidthisoneislongandinvalid",
            "shuttleapp",
        ] {
            let project_name = ProjectName::from_str(label);
            assert!(project_name.is_err(), "{:?} was ok", label);
        }
    }
}
