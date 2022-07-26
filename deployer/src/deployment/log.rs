use chrono::{DateTime, Utc};
use shuttle_common::BuildLog;

use super::{deploy_layer::to_build_log, State};

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

impl Log {
    pub fn into_build_log(self) -> Option<BuildLog> {
        to_build_log(&self.name, &self.timestamp, &self.fields)
    }
}
