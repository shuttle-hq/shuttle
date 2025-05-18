#![doc = include_str!("../README.md")]

#[cfg(feature = "postgres")]
mod postgres;

#[cfg(feature = "postgres")]
pub use postgres::Postgres;
#[cfg(feature = "opendal-postgres")]
pub use postgres::SerdeJsonOperator;
