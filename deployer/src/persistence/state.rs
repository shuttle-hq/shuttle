use strum::{Display, EnumString};

/// States a deployment can be in
#[derive(sqlx::Type, Debug, Display, Clone, Copy, EnumString, PartialEq, Eq)]
pub enum State {
    Queued,
    Building,
    Built,
    Running,
    Completed,
    Stopped,
    Crashed,
    Unknown,
}

impl Default for State {
    fn default() -> Self {
        Self::Unknown
    }
}

impl From<State> for shuttle_common::deployment::State {
    fn from(state: State) -> Self {
        match state {
            State::Queued => Self::Queued,
            State::Building => Self::Building,
            State::Built => Self::Built,
            State::Running => Self::Running,
            State::Completed => Self::Completed,
            State::Stopped => Self::Stopped,
            State::Crashed => Self::Crashed,
            State::Unknown => Self::Unknown,
        }
    }
}
