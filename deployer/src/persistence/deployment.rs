use std::{net::SocketAddr, str::FromStr};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use tracing::error;
use utoipa::ToSchema;
use uuid::Uuid;

use super::state::State;

// We are using `Option` for the additional `git_*` fields for backward compat.
#[derive(Clone, Debug, Default, Eq, PartialEq, ToSchema)]
pub struct Deployment {
    pub id: Uuid,
    pub service_id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
    pub address: Option<SocketAddr>,
    pub is_next: bool,
    pub git_commit_id: Option<String>,
    pub git_commit_msg: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
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
            is_next: row.try_get("is_next")?,
            git_commit_id: row.try_get("git_commit_id")?,
            git_commit_msg: row.try_get("git_commit_msg")?,
            git_branch: row.try_get("git_branch")?,
            git_dirty: row.try_get("git_dirty")?,
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
            git_commit_id: deployment.git_commit_id,
            git_commit_msg: deployment.git_commit_msg,
            git_branch: deployment.git_branch,
            git_dirty: deployment.git_dirty,
        }
    }
}

/// Update the details of a deployment
#[async_trait]
pub trait DeploymentUpdater: Clone + Send + Sync + 'static {
    type Err: std::error::Error + Send;

    /// Set the address for a deployment
    async fn set_address(&self, id: &Uuid, address: &SocketAddr) -> Result<(), Self::Err>;

    /// Set if a deployment is build on shuttle-next
    async fn set_is_next(&self, id: &Uuid, is_next: bool) -> Result<(), Self::Err>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct DeploymentState {
    pub id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
pub struct DeploymentRunnable {
    pub id: Uuid,
    pub service_name: String,
    pub service_id: Uuid,
    pub is_next: bool,
}
