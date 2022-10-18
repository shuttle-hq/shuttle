use std::fmt::Display;

use chrono::{DateTime, Utc};
use comfy_table::Color;
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::deployment::State;

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub id: Uuid,
    pub service_id: Uuid,
    pub state: State,
    pub last_update: DateTime<Utc>,
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} deployment '{}' is {}",
            self.last_update
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
                .dim(),
            self.id,
            self.state.to_string().cyan()
        )
    }
}

impl State {
    pub fn get_color(&self) -> Color {
        match self {
            State::Queued | State::Building | State::Built | State::Loading => Color::Cyan,
            State::Running => Color::Green,
            State::Completed | State::Stopped => Color::Blue,
            State::Crashed => Color::Red,
            State::Unknown => Color::Yellow,
        }
    }
}
