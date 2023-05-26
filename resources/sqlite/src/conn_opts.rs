use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use thiserror::Error;

mod auto_vacuum;
use auto_vacuum::*;

mod journal_mode;
use journal_mode::*;

mod locking_mode;
use locking_mode::*;

mod synchronous;
use synchronous::*;

#[derive(Error, Debug)]
pub enum SQLiteError {
    #[error("Failed to open db connection")]
    NoConnection,
}

/// Options to configure the SQLite database mirroring `sqlx::sqlite::SQLiteConnectOptions` for the options it exposes.
/// See [`sqlx::sqlite::SQLiteConnectOptions`](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SQLiteConnectOptions.html)
/// for the full documentation.
#[derive(Deserialize, Serialize)]
pub struct SQLiteConnOpts {
    // The full connection string we construct internally in `ResourceBuilder::output`.
    pub(crate) conn_str: String,
    // Mirrored options from the original.
    pub(crate) filename: Cow<'static, Path>,
    pub(crate) in_memory: bool,
    pub(crate) read_only: bool,
    pub(crate) create_if_missing: bool,
    pub(crate) shared_cache: bool,
    pub(crate) statement_cache_capacity: usize,
    pub(crate) busy_timeout: Duration,
    pub(crate) immutable: bool,
    pub(crate) vfs: Option<Cow<'static, str>>,

    pub(crate) pragmas: IndexMap<Cow<'static, str>, Option<Cow<'static, str>>>,

    pub(crate) command_channel_size: usize,
    pub(crate) row_channel_size: usize,

    pub(crate) serialized: bool,
}

impl Default for SQLiteConnOpts {
    fn default() -> Self {
        Self::new()
    }
}

impl SQLiteConnOpts {
    pub fn new() -> Self {
        let mut pragmas: IndexMap<Cow<'static, str>, Option<Cow<'static, str>>> = IndexMap::new();

        pragmas.insert("key".into(), None);

        pragmas.insert("cipher_plaintext_header_size".into(), None);

        pragmas.insert("cipher_salt".into(), None);

        pragmas.insert("kdf_iter".into(), None);

        pragmas.insert("cipher_kdf_algorithm".into(), None);

        pragmas.insert("cipher_use_hmac".into(), None);

        pragmas.insert("cipher_compatibility".into(), None);

        pragmas.insert("cipher_page_size".into(), None);

        pragmas.insert("cipher_hmac_algorithm".into(), None);

        pragmas.insert("page_size".into(), None);

        pragmas.insert("locking_mode".into(), None);

        pragmas.insert("journal_mode".into(), None);

        pragmas.insert("foreign_keys".into(), Some("ON".into()));

        pragmas.insert("synchronous".into(), None);

        pragmas.insert("auto_vacuum".into(), None);

        Self {
            conn_str: String::new(),
            filename: Cow::Borrowed(Path::new(":memory:")),
            in_memory: false,
            read_only: false,
            create_if_missing: false,
            shared_cache: false,
            statement_cache_capacity: 100,
            busy_timeout: Duration::from_secs(5),
            immutable: false,
            vfs: None,
            pragmas,
            serialized: false,
            command_channel_size: 50,
            row_channel_size: 50,
        }
    }

    /// Constructing the full connection string is handled internally. Use this to set the database to in memory mode.
    pub fn in_memory(mut self, on: bool) -> Self {
        self.in_memory = on;
        self
    }

    /// Set the database file name. Defaults to `default_db.sqlite`.
    pub fn filename(mut self, filename: impl AsRef<Path>) -> Self {
        self.filename = Cow::Owned(filename.as_ref().to_owned());
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.foreign_keys).
    pub fn foreign_keys(self, on: bool) -> Self {
        self.pragma("foreign_keys", if on { "ON" } else { "OFF" })
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.shared_cache).
    pub fn shared_cache(mut self, on: bool) -> Self {
        self.shared_cache = on;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.journal_mode).
    pub fn journal_mode(self, mode: SQLiteJournalMode) -> Self {
        self.pragma("journal_mode", mode.as_str())
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.locking_mode).
    pub fn locking_mode(self, mode: SQLiteLockingMode) -> Self {
        self.pragma("locking_mode", mode.as_str())
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.read_only).
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Different to the original implementation, Shuttle defaults to `true` here.
    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.create_if_missing).
    pub fn create_if_missing(mut self, create: bool) -> Self {
        self.create_if_missing = create;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.statement_cache_capacity).
    pub fn statement_cache_capacity(mut self, capacity: usize) -> Self {
        self.statement_cache_capacity = capacity;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.busy_timeout).
    pub fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.synchronous).
    pub fn synchronous(self, synchronous: SQLiteSynchronous) -> Self {
        self.pragma("synchronous", synchronous.as_str())
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.auto_vacuum).
    pub fn auto_vacuum(self, auto_vacuum: SQLiteAutoVacuum) -> Self {
        self.pragma("auto_vacuum", auto_vacuum.as_str())
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.page_size).
    pub fn page_size(self, page_size: u32) -> Self {
        self.pragma("page_size", page_size.to_string())
    }

    pub(crate) fn pragma<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    {
        self.pragmas.insert(key.into(), Some(value.into()));
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.immutable).
    pub fn immutable(mut self, immutable: bool) -> Self {
        self.immutable = immutable;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.serialized).
    pub fn serialized(mut self, serialized: bool) -> Self {
        self.serialized = serialized;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.command_buffer_size).
    pub fn command_buffer_size(mut self, size: usize) -> Self {
        self.command_channel_size = size;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.row_buffer_size).
    pub fn row_buffer_size(mut self, size: usize) -> Self {
        self.row_channel_size = size;
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.vfs).
    pub fn vfs(mut self, vfs_name: impl Into<Cow<'static, str>>) -> Self {
        self.vfs = Some(vfs_name.into());
        self
    }
}

impl From<&SQLiteConnOpts> for SqliteConnectOptions {
    fn from(opts: &SQLiteConnOpts) -> Self {
        let SQLiteConnOpts {
            conn_str,
            read_only,
            create_if_missing,
            shared_cache,
            statement_cache_capacity,
            busy_timeout,
            immutable,
            vfs,
            serialized,
            command_channel_size,
            row_channel_size,
            ..
        } = opts;

        let mut opts = SqliteConnectOptions::from_str(&conn_str)
            .expect("Failed to parse conn string")
            .read_only(*read_only)
            .create_if_missing(*create_if_missing)
            .shared_cache(*shared_cache)
            .statement_cache_capacity(*statement_cache_capacity)
            .busy_timeout(*busy_timeout)
            .immutable(*immutable)
            .serialized(*serialized)
            .command_buffer_size(*command_channel_size)
            .row_buffer_size(*row_channel_size);

        if let Some(vfs) = vfs {
            opts = opts.vfs(vfs.clone());
        }

        opts
    }
}
