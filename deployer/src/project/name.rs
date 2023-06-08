use std::{fmt::Formatter, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};
use shuttle_common::models::error::ErrorKind;

use crate::error::Error;

#[derive(Debug, sqlx::Type, Serialize, Clone, PartialEq, Eq, Hash)]
#[sqlx(transparent)]
pub struct ProjectName(String);

impl ProjectName {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_valid(&self) -> bool {
        let name = self.0.clone();

        fn is_valid_char(byte: u8) -> bool {
            matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-')
        }

        // each label in a hostname can be between 1 and 63 chars
        let is_invalid_length = name.len() > 63;

        !(name.bytes().any(|byte| !is_valid_char(byte))
            || name.ends_with('-')
            || name.starts_with('-')
            || name.is_empty()
            || is_invalid_length)
    }
}

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(<D::Error as serde::de::Error>::custom)
    }
}

impl FromStr for ProjectName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<shuttle_common::project::RawProjectName>()
            .map_err(|_| Error::from_kind(ErrorKind::InvalidProjectName))
            .map(|pn| Self(pn.to_string()))
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
