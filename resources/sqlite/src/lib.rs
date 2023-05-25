//! Shuttle resource providing an SQLite database. The database will be created in-process by the shuttle runtime.
//!
//! ## Example
//! ```rust
//! TODO: SIMPLE EXAMPLE
//! ```
//! ## Configuration
//! The database can be configured using `SQLiteConfig` that makes an almost complete subset of `sqlx::XXXX` available.
//! Notably, the full connection is not exposed but constructed internally.
//! Configuration can be done by passing a config struct or by passing individual fields. When both are being used, the one
//! that specified later wins.
//!
//! ```rust
//! TODO: CONFIG EXAMPLE
//! ```
use std::str::FromStr;

use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{Factory, ResourceBuilder, Type};
use sqlx::sqlite::SqliteConnectOptions;

pub use sqlx::SqlitePool;

pub mod conn_opts;
pub use conn_opts::*;

#[derive(Serialize)]
pub struct SQLite {
    config: SQLiteConnOpts,
}

impl SQLite {
    pub fn filename(mut self, filename: String) -> Self {
        self.config.filename = filename;
        self
    }

    pub fn config(mut self, config: SQLiteConnOpts) -> Self {
        self.config = config;
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
            config: SQLiteConnOpts::default(),
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
        let db_path = storage_path.join(&self.config.filename);
        self.config.conn_str = format!("sqlite:///{}", db_path.display());
        Ok(self.config)
    }

    /// Build this resource from its config output
    async fn build(build_data: &Self::Output) -> Result<sqlx::SqlitePool, shuttle_service::Error> {
        // debug!("Connecting to database at {db_path}");

        let opts = SqliteConnectOptions::from_str(&build_data.conn_str)
            .expect("Failed to parse conn string")
            .create_if_missing(true);

        let pool = sqlx::SqlitePool::connect_with(opts)
            .await
            .expect("Failed to create sqlite database");

        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        //
    }
}
