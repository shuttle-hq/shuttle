use chrono::{DateTime, Utc};
use ulid::Ulid;

use super::State;

#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct Log {
    pub id: Ulid,
    pub timestamp: DateTime<Utc>,
    pub state: State,
    pub level: Level,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub target: String,
    pub fields: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, sqlx::Type)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}