use std::fmt;

/// States a deployment can be in
#[derive(sqlx::Type, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(i32)]
pub enum State {
    Queued = 0,
    Building = 1,
    Built = 2,
    Running = 3,
    Error = 4,
    Unknown = 10,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Building => write!(f, "building"),
            Self::Built => write!(f, "built"),
            Self::Running => write!(f, "running"),
            Self::Error => write!(f, "error"),
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
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }
}
