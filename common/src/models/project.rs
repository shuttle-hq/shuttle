use comfy_table::Color;
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use strum::Display;

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub name: String,
    pub state: State,
}

#[derive(Clone, Debug, Deserialize, Display, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum State {
    Creating,
    Attaching,
    Starting,
    Started,
    Ready,
    Stopping,
    Stopped,
    Destroying,
    Destroyed,
    Errored,
}

impl Display for Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "project '{}' is {}",
            self.name,
            self.state.to_string().with(self.state.get_color())
        )
    }
}

impl State {
    pub fn get_color(&self) -> Color {
        match self {
            Self::Creating | Self::Attaching | Self::Starting | Self::Started => Color::Cyan,
            Self::Ready => Color::Green,
            Self::Stopped | Self::Stopping | Self::Destroying | Self::Destroyed => Color::Blue,
            Self::Errored => Color::Red,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct AdminResponse {
    pub project_name: String,
    pub account_name: String,
}
