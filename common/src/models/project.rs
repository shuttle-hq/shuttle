use std::fmt::Write;
use std::str::FromStr;

use chrono::{DateTime, SecondsFormat, Utc};
use comfy_table::{
    presets::{NOTHING, UTF8_BORDERS_ONLY},
    Attribute, Cell, Color, ContentArrangement, Table,
};
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use crate::deployment::DeploymentStateBeta;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[typeshare::typeshare]
pub struct ProjectCreateRequestBeta {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[typeshare::typeshare]
pub struct ProjectResponseBeta {
    pub id: String,
    /// Project owner
    pub user_id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub compute_tier: Option<ComputeTier>,
    /// State of the current deployment if one exists (something has been deployed).
    pub deployment_state: Option<DeploymentStateBeta>,
    /// URIs where running deployments can be reached
    pub uris: Vec<String>,
}

impl ProjectResponseBeta {
    pub fn to_string_colored(&self) -> String {
        let mut s = String::new();
        writeln!(&mut s, "{}", "Project info:".bold()).unwrap();
        writeln!(&mut s, "  Project ID: {}", self.id).unwrap();
        writeln!(&mut s, "  Project Name: {}", self.name).unwrap();
        writeln!(
            &mut s,
            "  Deployment Status: {}",
            self.deployment_state
                .as_ref()
                .map(|s| s.to_string_colored())
                .unwrap_or_else(|| "N/A".dark_grey().to_string())
        )
        .unwrap();
        writeln!(&mut s, "  Owner: {}", self.user_id).unwrap();
        writeln!(
            &mut s,
            "  Created: {}",
            self.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)
        )
        .unwrap();
        writeln!(&mut s, "  URIs:").unwrap();
        for uri in &self.uris {
            writeln!(&mut s, "    - {uri}").unwrap();
        }

        s
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[typeshare::typeshare]
pub struct ProjectListResponseBeta {
    pub projects: Vec<ProjectResponseBeta>,
}

/// Set wanted field(s) to Some to update those parts of the project
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
#[typeshare::typeshare]
pub struct ProjectUpdateRequestBeta {
    pub name: Option<String>,
    pub compute_tier: Option<ComputeTier>,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize, EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[typeshare::typeshare]
pub enum ComputeTier {
    #[default]
    XS,
    S,
    M,
    L,
    XL,
    XXL,
}

pub fn get_projects_table_beta(projects: &[ProjectResponseBeta], raw: bool) -> String {
    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Project ID", "Project Name", "Deployment Status"]);

    for project in projects {
        let state = project
            .deployment_state
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let color = project
            .deployment_state
            .as_ref()
            .map(|s| s.get_color())
            .unwrap_or_default();
        table.add_row(vec![
            Cell::new(&project.id).add_attribute(Attribute::Bold),
            Cell::new(&project.name),
            Cell::new(state)
                // Unwrap is safe because Color::from_str returns the color white if the argument is not a Color.
                .fg(Color::from_str(color).unwrap()),
        ]);
    }

    table.to_string()
}
