use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[cfg(feature = "display")]
use crossterm::style::Stylize;
#[cfg(feature = "display")]
use std::fmt::Write;

use super::deployment::DeploymentState;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectCreateRequest {
    #[cfg_attr(feature = "utoipa", schema(pattern = "^[a-z0-9-]{1,32}$"))]
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectResponse {
    pub id: String,
    /// Display name
    pub name: String,
    /// Project owner
    pub user_id: String,
    /// Team project belongs to
    pub team_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub compute_tier: Option<ComputeTier>,
    /// State of the current deployment if one exists (something has been deployed).
    pub deployment_state: Option<DeploymentState>,
    /// URIs where running deployments can be reached
    pub uris: Vec<String>,
}

impl ProjectResponse {
    #[cfg(feature = "display")]
    pub fn to_string_colored(&self) -> String {
        let mut s = String::new();
        writeln!(&mut s, "{}", "Project info:".bold()).unwrap();
        writeln!(&mut s, "  Project ID: {}", self.id).unwrap();
        writeln!(&mut s, "  Project Name: {}", self.name).unwrap();
        writeln!(&mut s, "  Owner: {}", self.user_id).unwrap();
        writeln!(
            &mut s,
            "  Team: {}",
            self.team_id.as_deref().unwrap_or("N/A")
        )
        .unwrap();
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}

/// Set wanted field(s) to Some to update those parts of the project
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUpdateRequest {
    /// Change display name
    #[cfg_attr(feature = "utoipa", schema(pattern = "^[a-z0-9-]{1,32}$"))]
    pub name: Option<String>,
    /// Transfer to other user
    #[cfg_attr(feature = "utoipa", schema(pattern = "^user_[A-Z0-9]{26}$"))]
    pub user_id: Option<String>,
    /// Transfer to a team
    #[cfg_attr(feature = "utoipa", schema(pattern = "^team_[A-Z0-9]{26}$"))]
    pub team_id: Option<String>,
    /// Transfer away from current team
    pub remove_from_team: Option<bool>,
    /// Project runtime configuration
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Display, Serialize, Deserialize, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum ComputeTier {
    #[default]
    XS,
    S,
    M,
    L,
    XL,
    XXL,

    /// Forward compatibility
    #[cfg(feature = "unknown-variants")]
    #[doc(hidden)]
    #[typeshare(skip)]
    #[serde(untagged, skip_serializing)]
    #[strum(default, to_string = "Unknown: {0}")]
    Unknown(String),
}

/// Sub-Response for the /user/me/usage backend endpoint
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageResponse {
    /// Show the build minutes clocked against this Project.
    pub build_minutes: ProjectUsageBuild,

    /// Show the VCPU used by this project on the container platform.
    pub vcpu: ProjectUsageVCPU,
}

/// Build Minutes subquery for the [`ProjectUsageResponse`] struct
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageBuild {
    /// Number of build minutes used by this project.
    pub used: u32,

    /// Limit of build minutes for this project, before additional charges are liable.
    pub limit: u32,
}

/// VCPU subquery for the [`ProjectUsageResponse`] struct
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageVCPU {
    /// The VCPU reserved for this project
    pub reserved: f32,

    /// Cost accrued from VCPU usage for this project
    pub billable_hours: f32,
}
