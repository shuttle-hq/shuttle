use super::{Built, Queued, State};

#[derive(sqlx::FromRow, serde::Serialize, Debug, PartialEq, Eq, Clone)]
pub struct DeploymentInfo {
    pub name: String,
    pub state: State,
}

impl From<&Queued> for DeploymentInfo {
    fn from(q: &Queued) -> Self {
        DeploymentInfo {
            name: q.name.clone(),
            state: State::Queued,
        }
    }
}

impl From<&Built> for DeploymentInfo {
    fn from(b: &Built) -> Self {
        DeploymentInfo {
            name: b.name.clone(),
            state: State::Built,
        }
    }
}
