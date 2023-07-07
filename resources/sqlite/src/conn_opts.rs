use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::{borrow::Cow, path::PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;

mod journal_mode;
pub use journal_mode::*;

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
/// `shared_cache` options. The following options are internally controlled by pragmas and hence also not exposed: `foreign_keys`, `locking_mode`, `auto_vacuum`, `page_size`.
#[derive(Debug, Deserialize, Serialize)]
pub struct ShuttleSqliteConnOpts {
    // Used for constructing the full connection string internally in `try_from`.
    pub(crate) storage_path: PathBuf,
    pub(crate) journal_mode: Option<ShuttleSqliteJournalMode>,
    pub(crate) synchronous: Option<ShuttleSqliteSynchronous>,
    // Mirrored options from the original.
    pub(crate) filename: Cow<'static, Path>,
    pub(crate) in_memory: bool,
    pub(crate) read_only: bool,
    pub(crate) create_if_missing: bool,
    pub(crate) statement_cache_capacity: usize,
    pub(crate) busy_timeout: Duration,
    pub(crate) immutable: bool,
    pub(crate) vfs: Option<Cow<'static, str>>,

    pub(crate) command_channel_size: usize,
    pub(crate) row_channel_size: usize,

    pub(crate) serialized: bool,
}

impl Default for ShuttleSqliteConnOpts {
    fn default() -> Self {
        Self::new()
    }
}

impl ShuttleSqliteConnOpts {
    pub fn new() -> Self {
        Self {
            storage_path: PathBuf::new(),
            filename: Cow::Borrowed(Path::new(":memory:")),
            journal_mode: None,
            synchronous: None,
            in_memory: false,
            read_only: false,
            // Different to what sqlx does.
            create_if_missing: true,
            statement_cache_capacity: 100,
            busy_timeout: Duration::from_secs(5),
            immutable: false,
            vfs: None,
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

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.journal_mode).
    pub fn journal_mode(mut self, journal_mode: ShuttleSqliteJournalMode) -> Self {
        self.journal_mode = Some(journal_mode);
        self
    }

    /// See [sqlx docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.synchronous).
    pub fn synchronous(mut self, synchronous: ShuttleSqliteSynchronous) -> Self {
        self.synchronous = Some(synchronous);
        self
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

impl TryFrom<&ShuttleSqliteConnOpts> for SqliteConnectOptions {
    type Error = shuttle_service::Error;

    fn try_from(opts: &ShuttleSqliteConnOpts) -> Result<Self, Self::Error> {
        let ShuttleSqliteConnOpts {
            storage_path,
            filename,
            journal_mode,
            synchronous,
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

        if let Some(journal_mode) = journal_mode {
            opts = opts.pragma("journal_mode", journal_mode.as_str());
        }

        if let Some(synchronous) = synchronous {
            opts = opts.pragma("synchronous", synchronous.as_str());
        }

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
