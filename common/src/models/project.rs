#[cfg(feature = "openapi")]
use crate::ulid_type;
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, CellAlignment, Color,
    ContentArrangement, Table,
};
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use strum::EnumString;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// Timeframe before a project is considered idle
pub const DEFAULT_IDLE_MINUTES: u64 = 30;

/// Function to set [DEFAULT_IDLE_MINUTES] as a serde default
pub const fn default_idle_minutes() -> u64 {
    DEFAULT_IDLE_MINUTES
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::models::project::Response))]
pub struct Response {
    #[cfg_attr(feature = "openapi", schema(schema_with = ulid_type))]
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::models::project::State))]
    pub state: State,
    pub idle_minutes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, EnumString)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::models::project::State))]
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
    Deleted,
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
        write!(f, r#"Project "{}" is {}"#, self.name, self.state)
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
                write!(f, "\tmessage: {message}")
            }
            State::Deleted => write!(f, "{}", "deleted".red()),
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
            Self::Errored { .. } | Self::Deleted => Color::Red,
        }
    }
}

/// Config when creating a new project
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub idle_minutes: u64,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::models::project::AdminResponse))]
pub struct AdminResponse {
    pub project_name: String,
    pub account_name: String,
}

pub fn get_table(projects: &Vec<Response>, page: u32) -> String {
    if projects.is_empty() {
        // The page starts at 1 in the CLI.
        if page <= 1 {
            format!(
                "{}\n",
                "No projects are linked to this account".yellow().bold()
            )
        } else {
            format!(
                "{}\n",
                "No more projects linked to this account".yellow().bold()
            )
        }
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
{table}

{}
"#,
            "More projects might be available on the next page using --page.".bold()
        )
    }
}
