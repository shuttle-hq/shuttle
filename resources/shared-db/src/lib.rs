#![doc = include_str!("../README.md")]

#[cfg(feature = "mongodb")]
mod mongo;
#[cfg(feature = "mongodb")]
pub use mongo::MongoDb;

#[cfg(feature = "postgres")]
mod postgres;

#[cfg(feature = "postgres")]
pub use postgres::Postgres;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum SharedDbOutput {
    Shared(shuttle_service::DatabaseReadyInfo),
    Local(String),
}
