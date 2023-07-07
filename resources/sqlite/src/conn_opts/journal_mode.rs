use std::str::FromStr;

use serde::{Deserialize, Serialize};

use sqlx::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ShuttleSqliteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl ShuttleSqliteJournalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ShuttleSqliteJournalMode::Delete => "DELETE",
            ShuttleSqliteJournalMode::Truncate => "TRUNCATE",
            ShuttleSqliteJournalMode::Persist => "PERSIST",
            ShuttleSqliteJournalMode::Memory => "MEMORY",
            ShuttleSqliteJournalMode::Wal => "WAL",
            ShuttleSqliteJournalMode::Off => "OFF",
        }
    }
}

impl Default for ShuttleSqliteJournalMode {
    fn default() -> Self {
        ShuttleSqliteJournalMode::Wal
    }
}

impl FromStr for ShuttleSqliteJournalMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "delete" => ShuttleSqliteJournalMode::Delete,
            "truncate" => ShuttleSqliteJournalMode::Truncate,
            "persist" => ShuttleSqliteJournalMode::Persist,
            "memory" => ShuttleSqliteJournalMode::Memory,
            "wal" => ShuttleSqliteJournalMode::Wal,
            "off" => ShuttleSqliteJournalMode::Off,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {:?} for `journal_mode`", s).into(),
                ));
            }
        })
    }
}
