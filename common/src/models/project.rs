use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[cfg(feature = "display")]
use crossterm::style::Stylize;
#[cfg(feature = "display")]
use std::fmt::Write;

use super::deployment::DeploymentStateBeta;

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
    #[cfg(feature = "display")]
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
            self.created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
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
