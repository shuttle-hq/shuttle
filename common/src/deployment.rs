use std::fmt::Display;

use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub id: Uuid,
    pub name: String,
    pub state: State,
    pub last_update: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Display, Serialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
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

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} deployment '{}' for {} is {}",
            self.last_update
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
                .bright_black(),
            self.id,
            self.name,
            self.state.to_string().cyan()
        )
    }
}
