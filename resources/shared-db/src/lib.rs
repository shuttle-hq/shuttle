#![doc = include_str!("../README.md")]
//! # [Shuttle Shared Databases](https://docs.shuttle.rs/resources/shuttle-shared-db)
//!
//! This plugin manages databases that are shared with other services on shuttle.
//! Your database will share the server with other users, but it will not 
//! be accessible by other users.

#[cfg(feature = "mongodb")]
mod mongo;
#[cfg(feature = "mongodb")]
pub use mongo::MongoDb;

#[cfg(any(feature = "postgres", feature = "postgres-rustls"))]
mod postgres;

#[cfg(any(feature = "postgres", feature = "postgres-rustls"))]
pub use postgres::Postgres;
