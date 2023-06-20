use sqlx::migrate::Migrator;
use tokio::task::JoinHandle;

use self::dal::Dal;
pub use self::error::Error as PersistenceError;
pub use self::service::Service;

pub mod dal;
mod error;
pub mod service;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Persistence<D: Dal + 'static> {
    dal: D,
}

impl<D: Dal + Send + Sync + 'static> Persistence<D> {
    pub async fn from_dal(dal: D) -> (Self, JoinHandle<()>) {
        // The logs are received on a non-async thread.
        // This moves them to an async thread
        let handle = tokio::spawn(async move {});

        let persistence = Self { dal };

        (persistence, handle)
    }

    pub fn dal(&self) -> &D {
        &self.dal
    }
}

#[cfg(test)]
mod tests {}
