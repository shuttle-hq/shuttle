use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum DeploymentState {
    Queued = 0,
    Building = 1,
    Built = 2,
    Running = 3,
    Error = 4,
}

impl fmt::Display for DeploymentState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DeploymentState::Queued => write!(f, "queued for deployment"),
            DeploymentState::Building => write!(f, "being built"),
            DeploymentState::Built => write!(f, "successfully built"),
            DeploymentState::Running => write!(f, "running"),
            DeploymentState::Error => write!(f, "error occurred"),
        }
    }
}
