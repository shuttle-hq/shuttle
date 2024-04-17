use chrono::{DateTime, Utc};
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS,
    presets::{NOTHING, UTF8_FULL},
    Attribute, Cell, CellAlignment, Color, ContentArrangement, Table,
};
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};
use uuid::Uuid;

use crate::deployment::{EcsState, State};

/// Max length of strings in the git metadata
pub const GIT_STRINGS_MAX_LENGTH: usize = 80;
/// Max HTTP body size for a deployment POST request
pub const CREATE_SERVICE_BODY_LIMIT: usize = 50_000_000;
const GIT_OPTION_NONE_TEXT: &str = "N/A";

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub id: Uuid,
    pub service_id: String,
    pub state: State,
    pub last_update: DateTime<Utc>,
    pub git_commit_id: Option<String>,
    pub git_commit_msg: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub struct EcsResponse {
    pub id: String,
    pub latest_deployment_state: EcsState,
    pub running_id: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub uri: String,
    pub git_commit_id: Option<String>,
    pub git_commit_msg: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
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
            self.state
                .to_string()
                // Unwrap is safe because Color::from_str returns the color white if the argument is not a Color.
                .with(crossterm::style::Color::from_str(self.state.get_color()).unwrap())
        )
    }
}

impl EcsResponse {
    pub fn colored_println(&self) {
        let running_deployment = self
            .running_id
            .as_ref()
            .map(|id| {
                format!(
                    "\nRunning deployment: '{}' - {} ({})",
                    id,
                    "running".to_string().with(
                        crossterm::style::Color::from_str(EcsState::Running.get_color()).unwrap()
                    ),
                    self.uri
                )
            })
            .unwrap_or_default();

        // Stringify the state.
        let latest_state = format!(
            "{}",
            self.latest_deployment_state
                .to_string()
                // Unwrap is safe because Color::from_str returns the color white if the argument is not a Color.
                .with(
                    crossterm::style::Color::from_str(self.latest_deployment_state.get_color())
                        .unwrap()
                )
        );

        let state_with_uri = match self.running_id {
            None => format!("{latest_state} ({})", self.uri),
            Some(_) => latest_state,
        };

        println!(
            "Current deployment: '{}' - {}{running_deployment}",
            self.id, state_with_uri
        )
    }
}

impl State {
    /// We return a &str rather than a Color here, since `comfy-table` re-exports
    /// crossterm::style::Color and we depend on both `comfy-table` and `crossterm`
    /// we may end up with two different versions of Color.
    pub fn get_color(&self) -> &str {
        match self {
            State::Queued | State::Building | State::Built | State::Loading => "cyan",
            State::Running => "green",
            State::Completed | State::Stopped => "blue",
            State::Crashed => "red",
            State::Unknown => "yellow",
        }
    }
}

impl EcsState {
    /// We return a &str rather than a Color here, since `comfy-table` re-exports
    /// crossterm::style::Color and we depend on both `comfy-table` and `crossterm`
    /// we may end up with two different versions of Color.
    pub fn get_color(&self) -> &str {
        match self {
            EcsState::InProgress => "cyan",
            EcsState::Running => "green",
            EcsState::Stopped => "blue",
        }
    }
}

pub fn get_deployments_table(
    deployments: &Vec<Response>,
    service_name: &str,
    page: u32,
    raw: bool,
    page_hint: bool,
) -> String {
    if deployments.is_empty() {
        // The page starts at 1 in the CLI.
        let mut s = if page <= 1 {
            "No deployments are linked to this service\n".to_string()
        } else {
            "No more deployments are linked to this service\n".to_string()
        };
        if !raw {
            s = s.yellow().bold().to_string();
        }

        s
    } else {
        let mut table = Table::new();

        if raw {
            table
                .load_preset(NOTHING)
                .set_content_arrangement(ContentArrangement::Disabled)
                .set_header(vec![
                    Cell::new("Deployment ID").set_alignment(CellAlignment::Left),
                    Cell::new("Status").set_alignment(CellAlignment::Left),
                    Cell::new("Last updated").set_alignment(CellAlignment::Left),
                    Cell::new("Commit ID").set_alignment(CellAlignment::Left),
                    Cell::new("Commit Message").set_alignment(CellAlignment::Left),
                    Cell::new("Branch").set_alignment(CellAlignment::Left),
                    Cell::new("Dirty").set_alignment(CellAlignment::Left),
                ]);
        } else {
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
                    Cell::new("Commit ID")
                        .set_alignment(CellAlignment::Center)
                        .add_attribute(Attribute::Bold),
                    Cell::new("Commit Message")
                        .set_alignment(CellAlignment::Center)
                        .add_attribute(Attribute::Bold),
                    Cell::new("Branch")
                        .set_alignment(CellAlignment::Center)
                        .add_attribute(Attribute::Bold),
                    Cell::new("Dirty")
                        .set_alignment(CellAlignment::Center)
                        .add_attribute(Attribute::Bold),
                ]);
        }

        for deploy in deployments.iter() {
            let truncated_commit_id = deploy
                .git_commit_id
                .as_ref()
                .map_or(String::from(GIT_OPTION_NONE_TEXT), |val| {
                    val.chars().take(7).collect()
                });

            let truncated_commit_msg = deploy
                .git_commit_msg
                .as_ref()
                .map_or(String::from(GIT_OPTION_NONE_TEXT), |val| {
                    val.chars().take(24).collect::<String>()
                });

            if raw {
                table.add_row(vec![
                    Cell::new(deploy.id),
                    Cell::new(&deploy.state),
                    Cell::new(deploy.last_update.format("%Y-%m-%dT%H:%M:%SZ")),
                    Cell::new(truncated_commit_id),
                    Cell::new(truncated_commit_msg),
                    Cell::new(
                        deploy
                            .git_branch
                            .as_ref()
                            .map_or(GIT_OPTION_NONE_TEXT, |val| val as &str),
                    ),
                    Cell::new(
                        deploy
                            .git_dirty
                            .map_or(String::from(GIT_OPTION_NONE_TEXT), |val| val.to_string()),
                    ),
                ]);
            } else {
                table.add_row(vec![
                    Cell::new(deploy.id),
                    Cell::new(&deploy.state)
                        // Unwrap is safe because Color::from_str returns the color white if str is not a Color.
                        .fg(Color::from_str(deploy.state.get_color()).unwrap())
                        .set_alignment(CellAlignment::Center),
                    Cell::new(deploy.last_update.format("%Y-%m-%dT%H:%M:%SZ"))
                        .set_alignment(CellAlignment::Center),
                    Cell::new(truncated_commit_id),
                    Cell::new(truncated_commit_msg),
                    Cell::new(
                        deploy
                            .git_branch
                            .as_ref()
                            .map_or(GIT_OPTION_NONE_TEXT, |val| val as &str),
                    ),
                    Cell::new(
                        deploy
                            .git_dirty
                            .map_or(String::from(GIT_OPTION_NONE_TEXT), |val| val.to_string()),
                    )
                    .set_alignment(CellAlignment::Center),
                ]);
            }
        }

        let formatted_table = format!("\nMost recent deployments for {service_name}\n{table}\n");
        if page_hint {
            format!(
                "{formatted_table}More deployments are available on the next page using `--page {}`\n",
                page + 1
            )
        } else {
            formatted_table
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct DeploymentRequest {
    /// Alpha: tar archive. Beta: zip archive.
    pub data: Vec<u8>,
    /// The cargo package name to compile and run. Required on beta.
    pub package_name: Option<String>,
    /// Ignored on beta.
    pub no_test: bool,
    pub git_commit_id: Option<String>,
    pub git_commit_msg: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}
