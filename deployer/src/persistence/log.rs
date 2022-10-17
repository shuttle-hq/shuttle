use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use shuttle_common::STATE_MESSAGE;
use uuid::Uuid;

use super::{deploy_layer::extract_message, State};

#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct Log {
    pub id: Uuid,
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

impl From<Log> for Option<shuttle_common::LogItem> {
    fn from(log: Log) -> Self {
        if log.state == State::Building {
            if let Value::String(str_value) = &log.fields {
                if str_value == STATE_MESSAGE {
                    return Some(log.into());
                }
            } else {
                let msg = extract_message(&log.fields)?;

                let item = shuttle_common::LogItem {
                    id: log.id,
                    state: log.state.into(),
                    timestamp: log.timestamp,
                    level: log.level.into(),
                    file: log.file,
                    line: log.line,
                    target: log.target,
                    fields: serde_json::to_vec(&json!({ "message": msg })).unwrap(),
                };

                return Some(item);
            }
        }

        Some(log.into())
    }
}

impl From<Log> for shuttle_common::LogItem {
    fn from(log: Log) -> Self {
        Self {
            id: log.id,
            state: log.state.into(),
            timestamp: log.timestamp,
            level: log.level.into(),
            file: log.file,
            line: log.line,
            target: log.target,
            fields: serde_json::to_vec(&log.fields).unwrap(),
        }
    }
}

impl From<Level> for shuttle_common::log::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::Trace => Self::Trace,
            Level::Debug => Self::Debug,
            Level::Info => Self::Info,
            Level::Warn => Self::Warn,
            Level::Error => Self::Error,
        }
    }
}

impl From<shuttle_common::log::Level> for Level {
    fn from(level: shuttle_common::log::Level) -> Self {
        match level {
            shuttle_common::log::Level::Trace => Self::Trace,
            shuttle_common::log::Level::Debug => Self::Debug,
            shuttle_common::log::Level::Info => Self::Info,
            shuttle_common::log::Level::Warn => Self::Warn,
            shuttle_common::log::Level::Error => Self::Error,
        }
    }
}
