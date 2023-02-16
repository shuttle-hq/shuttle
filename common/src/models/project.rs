use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, CellAlignment, Color,
    ContentArrangement, Table,
};
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub name: String,
    pub state: State,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Creating { recreate_count: usize },
    Attaching { recreate_count: usize },
    Recreating { recreate_count: usize },
    Starting { restart_count: usize },
    Restarting { restart_count: usize },
    Started,
    Ready,
    Stopping,
    Stopped,
    Rebooting,
    Destroying,
    Destroyed,
    Errored { message: String },
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Creating { .. }, Self::Creating { .. })
                | (Self::Attaching { .. }, Self::Attaching { .. })
                | (Self::Recreating { .. }, Self::Recreating { .. })
                | (Self::Starting { .. }, Self::Starting { .. })
                | (Self::Restarting { .. }, Self::Restarting { .. })
                | (Self::Started, Self::Started)
                | (Self::Ready, Self::Ready)
                | (Self::Stopping, Self::Stopping)
                | (Self::Stopped, Self::Stopped)
                | (Self::Rebooting, Self::Rebooting)
                | (Self::Destroying, Self::Destroying)
                | (Self::Destroyed, Self::Destroyed)
                | (Self::Errored { .. }, Self::Errored { .. })
        )
    }
}

impl Eq for State {}

impl Display for Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "project '{}' is {}", self.name, self.state)
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Creating { recreate_count } => {
                if *recreate_count > 0 {
                    write!(
                        f,
                        "{} (attempt {})",
                        "creating".dark_yellow(),
                        recreate_count
                    )
                } else {
                    write!(f, "{}", "creating".cyan())
                }
            }
            State::Attaching { recreate_count } => {
                if *recreate_count > 0 {
                    write!(
                        f,
                        "{} (attempt {})",
                        "attaching".dark_yellow(),
                        recreate_count
                    )
                } else {
                    write!(f, "{}", "attaching".cyan())
                }
            }
            State::Recreating { recreate_count } => {
                if *recreate_count > 0 {
                    write!(
                        f,
                        "{} (attempt {})",
                        "recreating".dark_yellow(),
                        recreate_count
                    )
                } else {
                    write!(f, "{}", "recreating".dark_yellow())
                }
            }
            State::Starting { restart_count } => {
                if *restart_count > 0 {
                    write!(
                        f,
                        "{} (attempt {})",
                        "starting".dark_yellow(),
                        restart_count
                    )
                } else {
                    write!(f, "{}", "starting".cyan())
                }
            }
            State::Restarting { restart_count } => {
                if *restart_count > 0 {
                    write!(
                        f,
                        "{} (attempt {})",
                        "restarting".dark_yellow(),
                        restart_count
                    )
                } else {
                    write!(f, "{}", "restarting".dark_yellow())
                }
            }
            State::Started => write!(f, "{}", "started".cyan()),
            State::Ready => write!(f, "{}", "ready".green()),
            State::Stopping => write!(f, "{}", "stopping".blue()),
            State::Stopped => write!(f, "{}", "stopped".blue()),
            State::Rebooting => write!(f, "{}", "rebooting".dark_yellow()),
            State::Destroying => write!(f, "{}", "destroying".blue()),
            State::Destroyed => write!(f, "{}", "destroyed".blue()),
            State::Errored { message } => {
                writeln!(f, "{}", "errored".red())?;
                write!(f, "\tmessage: {}", message)
            }
        }
    }
}

impl State {
    pub fn get_color(&self) -> Color {
        match self {
            Self::Creating { .. }
            | Self::Attaching { .. }
            | Self::Recreating { .. }
            | Self::Starting { .. }
            | Self::Restarting { .. }
            | Self::Started
            | Self::Rebooting => Color::Cyan,
            Self::Ready => Color::Green,
            Self::Stopped | Self::Stopping | Self::Destroying | Self::Destroyed => Color::Blue,
            Self::Errored { .. } => Color::Red,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct AdminResponse {
    pub project_name: String,
    pub account_name: String,
}

pub fn get_table(projects: &Vec<Response>) -> String {
    if projects.is_empty() {
        format!(
            "{}\n",
            "No projects are linked to this account".yellow().bold()
        )
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("Project Name").set_alignment(CellAlignment::Center),
                Cell::new("Status").set_alignment(CellAlignment::Center),
            ]);

        for project in projects.iter() {
            table.add_row(vec![
                Cell::new(&project.name),
                Cell::new(&project.state)
                    .fg(project.state.get_color())
                    .set_alignment(CellAlignment::Center),
            ]);
        }

        format!(
            r#"
These projects are linked to this account
{}
"#,
            table,
        )
    }
}
