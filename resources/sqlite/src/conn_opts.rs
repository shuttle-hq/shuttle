use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SQLiteError {
    #[error("Failed to open db connection")]
    NoConnection,
}

#[derive(Deserialize, Serialize)]
pub enum SQLiteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

#[derive(Deserialize, Serialize)]
pub enum SQLiteLockingMode {
    Normal,
    Exclusive,
}

#[derive(Deserialize, Serialize)]
pub enum SQLiteSynchronous {
    Off,
    Normal,
    Full,
    Extra,
}

#[derive(Deserialize, Serialize)]
pub enum SQLiteAutoVacuum {
    None,
    Full,
    Incremental,
}

#[derive(Deserialize, Serialize)]
pub struct SQLiteConnOpts {
    // TODO: Which other options to add?
    pub(crate) conn_str: String,
    pub filename: String,
    pub foreign_keys: bool,
    pub shared_cache: bool,
    pub journal_mode: Option<SQLiteJournalMode>,
    pub locking_mode: SQLiteLockingMode,
    pub read_only: bool,
    pub create_if_missing: bool,
    pub statement_cache_capacity: usize,
    pub busy_timeout: std::time::Duration,
    pub synchronous: SQLiteSynchronous,
    pub auto_vacuum: SQLiteAutoVacuum,
    pub page_size: u32,
    pub immutable: bool,
    pub serialized: bool,
    pub command_buffer_size: usize,
    pub row_buffer_size: usize,
    pub vfs: Option<Cow<'static, str>>,
}

impl Default for SQLiteConnOpts {
    fn default() -> Self {
        Self {
            conn_str: String::new(),
            filename: "default_db.sqlite".to_string(),
            foreign_keys: true,
            shared_cache: false,
            journal_mode: None,
            locking_mode: SQLiteLockingMode::Normal,
            read_only: false,
            create_if_missing: true,
            statement_cache_capacity: 100,
            busy_timeout: std::time::Duration::from_secs(5),
            synchronous: SQLiteSynchronous::Full,
            auto_vacuum: SQLiteAutoVacuum::None,
            page_size: 4096,
            immutable: true,
            serialized: false,
            command_buffer_size: 50,
            row_buffer_size: 50,
            vfs: None,
        }
    }
}

impl SQLiteConnOpts {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<SQLiteConnOpts> for SqliteConnectOptions {
    fn from(_value: SQLiteConnOpts) -> Self {
        todo!()
    }
}
