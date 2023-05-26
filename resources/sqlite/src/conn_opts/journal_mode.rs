use std::str::FromStr;

use serde::{Deserialize, Serialize};

use sqlx::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum SQLiteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl SQLiteJournalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SQLiteJournalMode::Delete => "DELETE",
            SQLiteJournalMode::Truncate => "TRUNCATE",
            SQLiteJournalMode::Persist => "PERSIST",
            SQLiteJournalMode::Memory => "MEMORY",
            SQLiteJournalMode::Wal => "WAL",
            SQLiteJournalMode::Off => "OFF",
        }
    }
}

impl Default for SQLiteJournalMode {
    fn default() -> Self {
        SQLiteJournalMode::Wal
    }
}

impl FromStr for SQLiteJournalMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "delete" => SQLiteJournalMode::Delete,
            "truncate" => SQLiteJournalMode::Truncate,
            "persist" => SQLiteJournalMode::Persist,
            "memory" => SQLiteJournalMode::Memory,
            "wal" => SQLiteJournalMode::Wal,
            "off" => SQLiteJournalMode::Off,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {:?} for `journal_mode`", s).into(),
                ));
            }
        })
    }
}
