//! Shuttle resource providing a SQLite database. The database will be created in-process by the shuttle runtime.
//!
//! ## Example
//! Simply annotate your main function to get a [`sqlx::SqlitePool`](https://docs.rs/sqlx/latest/sqlx/type.SqlitePool.html)
//! with default configuration.
//! Pass it to [`sqlx::query`](https://docs.rs/sqlx/latest/sqlx/macro.query.html) to interact with the database.
//! ```ignore
//! #[shuttle_runtime::main]
//! async fn axum(
//!     #[shuttle_sqlite::ShuttleSqlite] pool: shuttle_sqlite::SqlitePool,
//! ) -> shuttle_axum::ShuttleAxum {
//!     let _ = sqlx::query(
//!         "CREATE TABLE IF NOT EXISTS users(id int, name varchar(128), email varchar(128));",
//!     )
//!     .execute(&pool)
//!     .await
//!     .unwrap();
//! }
//! ```
//!
//! ## Configuration
//! The database can be configured using [`ShuttleSqliteConnOpts`] which mirrors sqlx's [`SqliteConnectOptions`](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html) for the
//! options it exposes.
//!
//! Construction of the full connection string is handled internally for security reasons and defaults to creating a
//! file-based database named `default_db.sqlite` with `create_if_missing == true`. Use the `filename` and/or
//! `in_memory` methods to configure the type of database created.
//!
//! See [`SqliteConnectOptions::new()`](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html#method.new)
//! for all default settings.
//!
//! ```ignore
//! #[shuttle_runtime::main]
//! async fn axum(
//!     #[shuttle_sqlite::ShuttleSqlite(opts = ShuttleSqliteConnOpts::new().filename("custom.sqlite"))] pool: shuttle_sqlite::SqlitePool,
//! ) -> shuttle_axum::ShuttleAxum { /* ... */ }
//! ```
//! Note that Shuttle does currently not support the `collation`, `thread_name`, `log_settings`, `pragma`, `extension`,
//! `shared_cache` options.
use std::path::Path;

use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{Factory, ResourceBuilder, Type};

mod conn_opts;
pub use conn_opts::*;

use sqlx::sqlite::SqliteConnectOptions;

/// Builder struct used to configure the database, e.g. `SQLite(opts = SQLiteConnOpts::new())`.
#[derive(Serialize)]
pub struct ShuttleSqlite {
    opts: ShuttleSqliteConnOpts,
}

impl ShuttleSqlite {
    pub fn opts(mut self, opts: ShuttleSqliteConnOpts) -> Self {
        self.opts = opts;
        self
    }

    pub fn filename(mut self, filename: impl AsRef<Path>) -> Self {
        self.opts = self.opts.filename(filename);
        self
    }

    pub fn in_memory(mut self, on: bool) -> Self {
        self.opts.in_memory = on;
        self
    }
}

#[async_trait]
impl ResourceBuilder<sqlx::SqlitePool> for ShuttleSqlite {
    const TYPE: Type = Type::EmbeddedDatabase;

    type Config = Self;

    type Output = ShuttleSqliteConnOpts;

    fn new() -> Self {
        Self {
            opts: ShuttleSqliteConnOpts::default(),
        }
    }

    fn config(&self) -> &Self::Config {
        &self
    }

    async fn output(
        mut self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        // We construct an absolute path using `storage_path` to prevent user access to other parts of the file system.
        let storage_path = factory.get_storage_path()?;
        self.opts.storage_path = storage_path;
        Ok(self.opts)
    }

    async fn build(build_data: &Self::Output) -> Result<sqlx::SqlitePool, shuttle_service::Error> {
        // This should never fail if our `try_from` is implemented correctly, which is guaranteed by our tests.
        let opts = SqliteConnectOptions::try_from(build_data)?;

        let pool = sqlx::SqlitePool::connect_with(opts)
            .await
            .map_err(|e| shuttle_service::Error::Database(e.to_string()))?;

        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use pretty_assertions::assert_eq;

    #[test]
    fn try_from_file_based() {
        let filename = "test.sqlite";
        let mut opts_sqlx =
            SqliteConnectOptions::from_str(format!("sqlite:///{filename}").as_str()).unwrap();
        opts_sqlx = opts_sqlx.create_if_missing(true); // Match our default setting
        let str_sqlx = format!("{:?}", opts_sqlx);

        let ours = ShuttleSqliteConnOpts::new().filename(&filename);
        let opts_from = SqliteConnectOptions::try_from(&ours).unwrap();
        let str_from = format!("{:?}", opts_from);

        assert_eq!(str_sqlx, str_from);
    }

    #[test]
    fn try_from_in_memory() {
        // `shared_cache` must be enabled to use a conn pool (instead of single conns) with an in-memory sqlite db.
        // This makes sure that `try_from` handles this correctly.
        let mut opts_sqlx = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        opts_sqlx = opts_sqlx.create_if_missing(true); // Match our default setting
        let str_sqlx = format!("{:?}", opts_sqlx);

        let ours = ShuttleSqliteConnOpts::new().in_memory(true);
        let opts_from = SqliteConnectOptions::try_from(&ours).unwrap();
        let str_from = format!("{:?}", opts_from);

        let re = regex::Regex::new(r#"filename:.*\d""#).unwrap();
        let str_sqlx = re.replace(&str_sqlx, "filename: ");
        let str_from = re.replace(&str_from, "filename: ");

        assert_eq!(str_sqlx, str_from);
    }
}
