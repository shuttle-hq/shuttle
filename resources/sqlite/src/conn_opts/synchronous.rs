use std::str::FromStr;

use serde::{Deserialize, Serialize};

use sqlx::error::Error;

#[derive(Deserialize, Serialize)]
pub enum SQLiteSynchronous {
    Off,
    Normal,
    Full,
    Extra,
}

impl SQLiteSynchronous {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SQLiteSynchronous::Off => "OFF",
            SQLiteSynchronous::Normal => "NORMAL",
            SQLiteSynchronous::Full => "FULL",
            SQLiteSynchronous::Extra => "EXTRA",
        }
    }
}

impl Default for SQLiteSynchronous {
    fn default() -> Self {
        SQLiteSynchronous::Full
    }
}

impl FromStr for SQLiteSynchronous {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "off" => SQLiteSynchronous::Off,
            "normal" => SQLiteSynchronous::Normal,
            "full" => SQLiteSynchronous::Full,
            "extra" => SQLiteSynchronous::Extra,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {:?} for `synchronous`", s).into(),
                ));
            }
        })
    }
}
