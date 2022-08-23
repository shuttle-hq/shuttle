use std::fmt;

/// States a deployment can be in
#[derive(sqlx::Type, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(i32)]
pub enum State {
    /// Deployment is queued to be build
    Queued = 0,

    /// Deployment is building, but is not done yet
    Building = 1,

    /// Deployment is built, but has not been started yet
    Built = 2,

    /// Deployment is running - ie. its thread is active
    Running = 3,

    /// Deployment was running, but stopped running all by itself. This is expected for things like background workers
    Completed = 4,

    /// Deployment was running, but has been stopped by the user.
    Stopped = 5,

    /// Something in the deployment process failed
    Crashed = 6,

    /// We never expect this state and entering this state should be considered a bug
    Unknown = 10,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Building => write!(f, "building"),
            Self::Built => write!(f, "built"),
            Self::Running => write!(f, "running"),
            State::Completed => write!(f, "completed"),
            State::Stopped => write!(f, "stopped"),
            State::Crashed => write!(f, "crashed"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::Unknown
    }
}

impl From<&dyn std::fmt::Debug> for State {
    fn from(input: &dyn std::fmt::Debug) -> Self {
        match format!("{input:?}").as_str() {
            "queued" => Self::Queued,
            "building" => Self::Building,
            "built" => Self::Built,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "stopped" => Self::Stopped,
            "crashed" => Self::Crashed,
            _ => Self::Unknown,
        }
    }
}
