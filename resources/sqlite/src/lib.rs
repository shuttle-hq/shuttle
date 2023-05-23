use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{database, Factory, ResourceBuilder, Type};
use thiserror::Error;
use tracing::debug;

pub use sqlx::SqlitePool;

// Builder struct
#[derive(Serialize)]
pub struct SQLite<'a> {
    db_name: &'a str,
}

// Resource struct
#[derive(Deserialize, Serialize, Clone)]
pub struct SQLiteInstance {
    db_path: PathBuf,
}

#[derive(Error, Debug)]
pub enum SQLiteError {
    #[error("Failed to open db connection")]
    NoConnection,
}

impl<'a> SQLite<'a> {
    pub fn db_name(mut self, db_name: &'a str) -> Self {
        self.db_name = db_name;
        Self { db_name }
    }
}

#[async_trait]
impl<'a> ResourceBuilder<sqlx::SqlitePool> for SQLite<'a> {
    /// The type of resource this creates
    const TYPE: Type = Type::Database(database::Type::Filesystem);

    /// The internal config being constructed by this builder. This will be used to find cached [Self::Output].
    type Config = Self;

    /// The output type used to build this resource later
    type Output = SQLiteInstance;

    /// Create a new instance of this resource builder
    fn new() -> Self {
        Self {
            db_name: "sqlite_db",
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
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        // TODO: Construct db name from service name + something?
        let _service_name = factory.get_service_name();

        let storage_path = factory.get_storage_path()?;
        let db_path = storage_path.join(self.db_name);

        Ok(SQLiteInstance { db_path })
    }

    /// Build this resource from its config output
    async fn build(build_data: &Self::Output) -> Result<sqlx::SqlitePool, shuttle_service::Error> {
        let db_path = &build_data.db_path;
        debug!("Connecting to database at {db_path:?}");

        let pool = sqlx::SqlitePool::connect(&format!("sqlite:///{db_path:?}"))
            .await
            .unwrap();
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
