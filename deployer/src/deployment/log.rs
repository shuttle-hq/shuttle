use chrono::{DateTime, Utc};

use super::State;

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Log {
    pub name: String,
    pub timestamp: DateTime<Utc>,
    pub state: State,
    pub level: Level,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub fields: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, sqlx::Type)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
