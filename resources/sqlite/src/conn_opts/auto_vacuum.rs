use std::str::FromStr;

use serde::{Deserialize, Serialize};

use sqlx::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum SQLiteAutoVacuum {
    None,
    Full,
    Incremental,
}

impl SQLiteAutoVacuum {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SQLiteAutoVacuum::None => "NONE",
            SQLiteAutoVacuum::Full => "FULL",
            SQLiteAutoVacuum::Incremental => "INCREMENTAL",
        }
    }
}

impl Default for SQLiteAutoVacuum {
    fn default() -> Self {
        SQLiteAutoVacuum::None
    }
}

impl FromStr for SQLiteAutoVacuum {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "none" => SQLiteAutoVacuum::None,
            "full" => SQLiteAutoVacuum::Full,
            "incremental" => SQLiteAutoVacuum::Incremental,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {:?} for `auto_vacuum`", s).into(),
                ));
            }
        })
    }
}
