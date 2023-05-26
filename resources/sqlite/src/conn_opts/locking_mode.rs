use std::str::FromStr;

use serde::{Deserialize, Serialize};

use sqlx::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum SQLiteLockingMode {
    Normal,
    Exclusive,
}

impl SQLiteLockingMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SQLiteLockingMode::Normal => "NORMAL",
            SQLiteLockingMode::Exclusive => "EXCLUSIVE",
        }
    }
}

impl Default for SQLiteLockingMode {
    fn default() -> Self {
        SQLiteLockingMode::Normal
    }
}

impl FromStr for SQLiteLockingMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "normal" => SQLiteLockingMode::Normal,
            "exclusive" => SQLiteLockingMode::Exclusive,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {:?} for `locking_mode`", s).into(),
                ));
            }
        })
    }
}
