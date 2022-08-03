use chrono::{DateTime, Utc};
use serde_json::Value;
use shuttle_common::{deployment, log::StreamLog};
use uuid::Uuid;

use super::{deploy_layer::extract_message, State};

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Log {
    pub id: Uuid,
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
    pub fn into_stream_log(self) -> Option<StreamLog> {
        let (state, message) = if let Value::String(str_value) = &self.fields {
            if str_value == "NEW STATE" {
                match self.state {
                    State::Queued => Some((deployment::State::Queued, None)),
                    State::Building => Some((deployment::State::Building, None)),
                    State::Built => Some((deployment::State::Built, None)),
                    State::Running => Some((deployment::State::Running, None)),
                    State::Completed => Some((deployment::State::Completed, None)),
                    State::Stopped => Some((deployment::State::Stopped, None)),
                    State::Crashed => Some((deployment::State::Crashed, None)),
                    State::Unknown => Some((deployment::State::Unknown, None)),
                }
            } else {
                None
            }
        } else {
            match self.state {
                State::Building => {
                    let msg = extract_message(&self.fields)?;
                    Some((deployment::State::Building, Some(msg)))
                }
                _ => None,
            }
        }?;

        Some(StreamLog {
            id: self.id,
            timestamp: self.timestamp,
            state,
            message,
        })
    }
}
