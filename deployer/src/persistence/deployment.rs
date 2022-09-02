use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::state::State;

#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct Deployment {
    pub id: Uuid,
    pub service_id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
}

impl From<Deployment> for shuttle_common::deployment::Response {
    fn from(deployment: Deployment) -> Self {
        shuttle_common::deployment::Response {
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
}

#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
pub struct DeploymentRunnable {
    pub id: Uuid,
    pub service_name: String,
    pub service_id: Uuid,
}
