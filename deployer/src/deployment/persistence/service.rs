use std::net::Ipv4Addr;
use std::str::FromStr;

use sqlx::types::Json as SqlxJson;
use sqlx::Row;
use sqlx::{sqlite::SqliteRow, FromRow};
use tracing::error;
use ulid::Ulid;

use super::error::Error;
use crate::project::service::ServiceState;

// User service model from persistence.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub id: Ulid,
    pub name: String,
    pub state_variant: String,
    pub state: ServiceState,
}

impl FromRow<'_, SqliteRow> for Service {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: Ulid::from_string(row.try_get("id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            name: row.try_get("name")?,
            state_variant: row.try_get("state_variant")?,
            state: row.try_get::<SqlxJson<ServiceState>, _>("state")?.0,
        })
    }
}
