use serde::{Deserialize, Serialize};

use sqlx::sqlite::SqliteJournalMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ShuttleSqliteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl Default for ShuttleSqliteJournalMode {
    fn default() -> Self {
        ShuttleSqliteJournalMode::Wal
    }
}

impl From<&ShuttleSqliteJournalMode> for SqliteJournalMode {
    fn from(value: &ShuttleSqliteJournalMode) -> Self {
        match value {
            ShuttleSqliteJournalMode::Delete => SqliteJournalMode::Delete,
            ShuttleSqliteJournalMode::Truncate => SqliteJournalMode::Truncate,
            ShuttleSqliteJournalMode::Persist => SqliteJournalMode::Persist,
            ShuttleSqliteJournalMode::Memory => SqliteJournalMode::Memory,
            ShuttleSqliteJournalMode::Wal => SqliteJournalMode::Wal,
            ShuttleSqliteJournalMode::Off => SqliteJournalMode::Off,
        }
    }
}
