use shuttle_common::deployment;
use strum::{Display, EnumString};

/// States a deployment can be in
#[derive(sqlx::Type, Debug, Display, Clone, Copy, EnumString, PartialEq, Eq, serde::Serialize)]
pub enum State {
    /// Deployment is queued to be build
    Queued,

    /// Deployment is building, but is not done yet
    Building,

    /// Deployment is built, but has not been started yet
    Built,

    /// Deployment is running - ie. its thread is active
    Running,

    /// Deployment was running, but stopped running all by itself. This is expected for things like background workers
    Completed,

    /// Deployment was running, but has been stopped by the user.
    Stopped,

    /// Something in the deployment process failed
    Crashed,

    /// We never expect this state and entering this state should be considered a bug
    Unknown,
}

impl Default for State {
    fn default() -> Self {
        Self::Unknown
    }
}

impl From<deployment::State> for State {
    fn from(state: deployment::State) -> Self {
        match state {
            deployment::State::Queued => Self::Queued,
            deployment::State::Building => Self::Building,
            deployment::State::Built => Self::Built,
            deployment::State::Running => Self::Running,
            deployment::State::Completed => Self::Completed,
            deployment::State::Stopped => Self::Stopped,
            deployment::State::Crashed => Self::Crashed,
            deployment::State::Unknown => Self::Unknown,
        }
    }
}
