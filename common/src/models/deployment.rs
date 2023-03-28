use std::fmt::Display;

use chrono::{DateTime, Utc};
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Attribute, Cell, CellAlignment, Color,
    ContentArrangement, Table,
};
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

pub fn get_deployments_table(deployments: &Vec<Response>, service_name: &str) -> String {
    if deployments.is_empty() {
        format!(
            "{}\n",
            "No deployments are linked to this service".yellow().bold()
        )
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("Deployment ID")
                    .set_alignment(CellAlignment::Center)
                    .add_attribute(Attribute::Bold),
                Cell::new("Status")
                    .set_alignment(CellAlignment::Center)
                    .add_attribute(Attribute::Bold),
                Cell::new("Last updated")
                    .set_alignment(CellAlignment::Center)
                    .add_attribute(Attribute::Bold),
            ]);

        for deploy in deployments.iter() {
            table.add_row(vec![
                Cell::new(deploy.id),
                Cell::new(&deploy.state)
                    .fg(deploy.state.get_color())
                    .set_alignment(CellAlignment::Center),
                Cell::new(deploy.last_update.format("%Y-%m-%dT%H:%M:%SZ"))
                    .set_alignment(CellAlignment::Center),
            ]);
        }

        format!(
            r#"
Most recent {} for {}
{}
"#,
            "deployments".bold(),
            service_name,
            table,
        )
    }
}
