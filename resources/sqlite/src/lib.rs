//! Shuttle resource providing a SQLite database. The database will be created in-process by the shuttle runtime.
//!
//! ## Example
//! Simply annotate your main function to get a [`sqlx::SqlitePool`](https://docs.rs/sqlx/latest/sqlx/type.SqlitePool.html)
//! with default configuration.
//! Pass it to [`sqlx::query`](https://docs.rs/sqlx/latest/sqlx/macro.query.html) to interact with the database.
//! ```ignore
//! #[shuttle_runtime::main]
//! async fn axum(
//!     #[shuttle_sqlite::SQLite] pool: shuttle_sqlite::SqlitePool,
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
//! The database can be configured using [`SQLiteConnOpts`] which mirrors sqlx's [`SqliteConnectOptions`](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html) for the
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
//!     #[shuttle_sqlite::SQLite(opts = SQLiteConnOpts::new().filename("custom.sqlite"))] pool: shuttle_sqlite::SqlitePool,
//! ) -> shuttle_axum::ShuttleAxum { /* ... */ }
//! ```
//! Note that Shuttle does currently not support the `collation`, `thread_name`, `log_settings`, `pragma`, `extension`,
//! `shared_cache` options.
use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{Factory, ResourceBuilder, Type};

mod conn_opts;
pub use conn_opts::*;

use sqlx::sqlite::SqliteConnectOptions;
/// The [`sqlx::SqlitePool`](https://docs.rs/sqlx/latest/sqlx/type.SqlitePool.html) that is being returned to the user.
///
pub use sqlx::SqlitePool;

/// Builder struct used to configure the database, e.g. `SQLite(opts = SQLiteConnOpts::new())`.
#[derive(Serialize)]
pub struct SQLite {
    opts: SQLiteConnOpts,
}

impl SQLite {
    pub fn opts(mut self, opts: SQLiteConnOpts) -> Self {
        self.opts = opts;
        self
    }

    pub fn in_memory(mut self, on: bool) -> Self {
        self.opts.in_memory = on;
        self
    }
}

#[async_trait]
impl ResourceBuilder<sqlx::SqlitePool> for SQLite {
    /// The type of resource this creates
    const TYPE: Type = Type::EmbeddedDatabase;

    /// The internal config being constructed by this builder. This will be used to find cached [Self::Output].
    type Config = Self;

    /// The output type used to build this resource later
    type Output = SQLiteConnOpts;

    /// Create a new instance of this resource builder
    fn new() -> Self {
        Self {
            opts: SQLiteConnOpts::default(),
        }
    }

    /// Get the internal config state of the builder
    ///
    /// If the exact same config was returned by a previous deployement that used this resource, then [Self::output()]
    /// will not be called to get the builder output again. Rather the output state of the previous deployment
    /// will be passed to [Self::build()].
    fn config(&self) -> &Self::Config {
        &self
    }

    /// Get the config output of this builder
    ///
    /// This method is where the actual resource provisioning should take place and is expected to take the longest. It
    /// can at times even take minutes. That is why the output of this method is cached and calling this method can be
    /// skipped as explained in [Self::config()].
    async fn output(
        mut self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        // We construct an absolute path using `storage_path` to prevent user access to other parts of the file system.
        let storage_path = factory.get_storage_path()?;
        self.opts.storage_path = storage_path;
        Ok(self.opts)
    }

    /// Build this resource from its config output
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
    use assert_json_diff::assert_json_eq;

    use super::*;
    use std::str::FromStr;

    #[test]
    fn try_from_file_based() {
        let filename = "test.sqlite";
        let mut opts_sqlx =
            SqliteConnectOptions::from_str(format!("sqlite:///{filename}").as_str()).unwrap();
        opts_sqlx = opts_sqlx.create_if_missing(true); // Match our default setting
        let str_sqlx = format!("{:?}", opts_sqlx);

        let ours = SQLiteConnOpts::new().filename(&filename);
        let opts_from = SqliteConnectOptions::try_from(&ours).unwrap();
        let str_from = format!("{:?}", opts_from);

        let json_sqlx = serde_json::json!(str_sqlx);
        let json_from = serde_json::json!(str_from);

        assert_json_eq!(json_sqlx, json_from);
    }

    #[test]
    fn try_from_in_memory() {
        // `shared_cache` must be enabled to use a conn pool (instead of single conns) with an in-memory sqlite db.
        // This makes sure that `try_from` handles this correctly.
        let mut opts_sqlx = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        opts_sqlx = opts_sqlx.create_if_missing(true); // Match our default setting
        let str_sqlx = format!("{:?}", opts_sqlx);

        let ours = SQLiteConnOpts::new().in_memory(true);
        let opts_from = SqliteConnectOptions::try_from(&ours).unwrap();
        let str_from = format!("{:?}", opts_from);

        let re = regex::Regex::new(r#"filename:.*\d""#).unwrap();
        let str_sqlx = re.replace(&str_sqlx, "filename: ");
        let str_from = re.replace(&str_from, "filename: ");

        let json_sqlx = serde_json::json!(str_sqlx);
        let json_from = serde_json::json!(str_from);

        assert_json_eq!(json_sqlx, json_from);
    }
}
