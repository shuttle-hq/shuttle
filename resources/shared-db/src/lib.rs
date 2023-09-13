#![doc = include_str!("../README.md")]

#[cfg(feature = "mongodb")]
mod mongo;
#[cfg(feature = "mongodb")]
pub use mongo::MongoDb;

#[cfg(any(feature = "postgres", feature = "postgres-rustls"))]
mod postgres;

#[cfg(any(feature = "postgres", feature = "postgres-rustls"))]
pub use postgres::Postgres;
