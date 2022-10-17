use std::{net::SocketAddr, str::FromStr};

use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use tracing::error;
use uuid::Uuid;

use super::state::State;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deployment {
    pub id: Uuid,
    pub service_id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
    pub address: Option<SocketAddr>,
}

impl FromRow<'_, SqliteRow> for Deployment {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let address = if let Some(address_str) = row.try_get::<Option<String>, _>("address")? {
            match SocketAddr::from_str(&address_str) {
                Ok(address) => Some(address),
                Err(err) => {
                    error!(error = %err, "failed to parse address from DB");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            id: row.try_get("id")?,
            service_id: row.try_get("service_id")?,
            state: row.try_get("state")?,
            last_update: row.try_get("last_update")?,
            address,
        })
    }
}

impl From<Deployment> for shuttle_common::models::deployment::Response {
    fn from(deployment: Deployment) -> Self {
        shuttle_common::models::deployment::Response {
            id: deployment.id,
            service_id: deployment.service_id,
            state: deployment.state.into(),
            last_update: deployment.last_update,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct DeploymentState {
    pub id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
    pub address: Option<SocketAddr>,
}

#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
pub struct DeploymentRunnable {
    pub id: Uuid,
    pub service_name: String,
    pub service_id: Uuid,
}
