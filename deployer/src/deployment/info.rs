use chrono::{DateTime, Utc};

use super::{Built, State};

#[derive(sqlx::FromRow, serde::Serialize, Debug, PartialEq, Eq, Clone)]
pub struct DeploymentState {
    pub id: String,
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
