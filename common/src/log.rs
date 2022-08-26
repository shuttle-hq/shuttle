use std::fmt::Display;
use std::fmt::Write;

use chrono::{DateTime, Local, Utc};
use crossterm::style::{StyledContent, Stylize};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::deployment::State;

pub const STATE_MESSAGE: &str = "NEW STATE";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildLogStream {
    pub id: Uuid,
    pub state: State,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Item {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub state: State,
    pub level: Level,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub target: String,
    pub fields: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let datetime: DateTime<Local> = DateTime::from(self.timestamp);

        let message = match &self.fields {
            serde_json::Value::String(str_value) if str_value == STATE_MESSAGE => {
                write!(f, "\n")?;
                format!("Entering {} state", self.state)
                    .bold()
                    .blue()
                    .to_string()
            }
            serde_json::Value::Object(map) => {
                let mut simple = None;
                let mut extra = vec![];

                for (key, value) in map.iter() {
                    match key.as_str() {
                        "message" => simple = value.as_str(),
                        _ => extra.push(format!("{key}={value}")),
                    }
                }

                let mut output = if extra.is_empty() {
                    String::new()
                } else {
                    format!("{{{}}} ", extra.join(" "))
                };

                if !self.target.is_empty() {
                    let target = format!("{}:", self.target).dim();
                    write!(output, "{target} ")?;
                }

                if let Some(msg) = simple {
                    write!(output, "{msg}")?;
                }

                output
            }
            other => other.to_string(),
        };

        write!(
            f,
            "{} {} {}",
            datetime.format("%Y-%m-%dT%H:%M:%S.%fZ").to_string().dim(),
            self.level.get_colored(),
            message
        )
    }
}

impl Level {
    fn get_colored(&self) -> StyledContent<&str> {
        match self {
            Level::Trace => "TRACE".magenta(),
            Level::Debug => "DEBUG".blue(),
            Level::Info => " INFO".green(),
            Level::Warn => " WARN".yellow(),
            Level::Error => "ERROR".red(),
        }
    }
}
