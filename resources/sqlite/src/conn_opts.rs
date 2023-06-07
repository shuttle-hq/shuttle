use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::{borrow::Cow, path::PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;

mod auto_vacuum;
pub use auto_vacuum::*;

mod journal_mode;
pub use journal_mode::*;

mod locking_mode;
pub use locking_mode::*;

mod synchronous;
pub use synchronous::*;
use tracing::debug;

/// Options to configure the SQLite database mirroring sqlx's [`SqliteConnectOptions`](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html)
/// for the options it exposes, see their docs for reference.
///
/// Construction of the full connection string is handled internally for security reasons and defaults to creating a
/// file-based database named `default_db.sqlite` with `create_if_missing == true`. Use the `filename` and/or
/// `in_memory` methods to configure the type of database created.
///
/// Note that Shuttle does currently not support the `collation`, `thread_name`, `log_settings`, `pragma`, `extension`,
/// `shared_cache` options.
#[derive(Debug, Deserialize, Serialize)]
pub struct SQLiteConnOpts {
    // Used for constructing the full connection string internally in `try_from`.
    pub(crate) storage_path: PathBuf,
    // Mirrored options from the original.
    pub(crate) filename: Cow<'static, Path>,
    pub(crate) in_memory: bool,
    pub(crate) read_only: bool,
    pub(crate) create_if_missing: bool,
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
            storage_path: PathBuf::new(),
            filename: Cow::Borrowed(Path::new(":memory:")),
            in_memory: false,
            read_only: false,
            // Different to what sqlx does.
            create_if_missing: true,
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

    /// Use this to set the database to in memory mode.
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

impl TryFrom<&SQLiteConnOpts> for SqliteConnectOptions {
    type Error = shuttle_service::Error;

    fn try_from(opts: &SQLiteConnOpts) -> Result<Self, Self::Error> {
        let SQLiteConnOpts {
            storage_path,
            filename,
            in_memory,
            read_only,
            create_if_missing,
            statement_cache_capacity,
            busy_timeout,
            immutable,
            vfs,
            serialized,
            command_channel_size,
            row_channel_size,
            ..
        } = opts;

        let db_path = storage_path.join(&filename);

        let conn_str = match in_memory {
            true => "sqlite::memory:".to_string(),
            false => format!("sqlite:///{}", db_path.display()),
        };

        debug!("Creating SqliteConnectOptions from {:?}", conn_str);

        let mut opts = SqliteConnectOptions::from_str(&conn_str)
            .map_err(|e| shuttle_service::Error::Database(e.to_string()))?
            .read_only(*read_only)
            .create_if_missing(*create_if_missing)
            .statement_cache_capacity(*statement_cache_capacity)
            .busy_timeout(*busy_timeout)
            .immutable(*immutable)
            .serialized(*serialized)
            .command_buffer_size(*command_channel_size)
            .row_buffer_size(*row_channel_size);

        if let Some(vfs) = vfs {
            opts = opts.vfs(vfs.clone());
        }

        // `shared_cache` must be enabled to use a conn pool (instead of single conns) with an in-memory sqlite db.
        if *in_memory {
            opts = opts.shared_cache(true);
        }

        Ok(opts)
    }
}
