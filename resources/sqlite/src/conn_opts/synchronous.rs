use std::str::FromStr;

use serde::{Deserialize, Serialize};

use sqlx::error::Error;

#[derive(Debug, Deserialize, Serialize)]
pub enum ShuttleSqliteSynchronous {
    Off,
    Normal,
    Full,
    Extra,
}

impl ShuttleSqliteSynchronous {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ShuttleSqliteSynchronous::Off => "OFF",
            ShuttleSqliteSynchronous::Normal => "NORMAL",
            ShuttleSqliteSynchronous::Full => "FULL",
            ShuttleSqliteSynchronous::Extra => "EXTRA",
        }
    }
}

impl Default for ShuttleSqliteSynchronous {
    fn default() -> Self {
        ShuttleSqliteSynchronous::Full
    }
}

impl FromStr for ShuttleSqliteSynchronous {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "off" => ShuttleSqliteSynchronous::Off,
            "normal" => ShuttleSqliteSynchronous::Normal,
            "full" => ShuttleSqliteSynchronous::Full,
            "extra" => ShuttleSqliteSynchronous::Extra,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {:?} for `synchronous`", s).into(),
                ));
            }
        })
    }
}
