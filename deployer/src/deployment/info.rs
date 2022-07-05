use chrono::{DateTime, Utc};

use super::{Built, Queued, State};

#[derive(sqlx::FromRow, serde::Serialize, Debug, PartialEq, Eq, Clone)]
pub struct DeploymentState {
    pub id: String,
    pub state: State,
    pub last_update: DateTime<Utc>,
}

impl From<&Queued> for DeploymentState {
    fn from(q: &Queued) -> Self {
        DeploymentState {
            id: q.id.clone(),
            state: State::Queued,
            last_update: Utc::now(),
        }
    }
}

impl From<&Built> for DeploymentState {
    fn from(b: &Built) -> Self {
        DeploymentState {
            id: b.id.clone(),
            state: State::Built,
            last_update: Utc::now(),
        }
    }
}
