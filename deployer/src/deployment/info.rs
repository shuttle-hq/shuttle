use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{Built, State};

#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
pub struct DeploymentState {
    pub id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
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
