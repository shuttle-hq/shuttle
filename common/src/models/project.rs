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

        // Emit the user's instance size
        if let Some(compute_tier) = &self.compute_tier {
            writeln!(&mut s, "  Instance size: {}", compute_tier).unwrap_or_default();
            writeln!(
                &mut s,
                "  Configured via `#[shuttle_runtime::main(instance_size = {})]`",
                serde_json::to_string(compute_tier).unwrap()
            )
            .unwrap_or_default();
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
    #[strum(to_string = "Basic (0.25 vCPU / 0.5 GB)")]
    XS,
    #[strum(to_string = "Small (0.5 vCPU / 1 GB)")]
    S,
    #[strum(to_string = "Medium (1 vCPU / 2 GB)")]
    M,
    #[strum(to_string = "Large (2 vCPU / 4 GB)")]
    L,
    #[strum(to_string = "X Large (4 vCPU / 8 GB)")]
    XL,
    #[strum(to_string = "XX Large (8 vCPU / 16 GB)")]
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
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageResponse {
    /// Show the build minutes clocked against this Project.
    pub build_minutes: ProjectUsageBuild,

    /// Show the VCPU used by this project on the container platform.
    pub vcpu: ProjectUsageVCPU,

    /// Daily usage breakdown for this project
    pub daily: Vec<ProjectUsageDaily>,
}

/// Build Minutes subquery for the [`ProjectUsageResponse`] struct
#[derive(Debug, Default, Deserialize, Serialize, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageBuild {
    /// Number of build minutes used by this project.
    pub used: u32,

    /// Limit of build minutes for this project, before additional charges are liable.
    pub limit: u32,
}

/// VCPU subquery for the [`ProjectUsageResponse`] struct
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageVCPU {
    /// Used reserved VCPU hours for a project.
    pub reserved_hours: f32,

    /// Used VCPU hours beyond the included reserved VCPU hours for a project.
    pub billable_hours: f32,
}

// Add this new struct for daily usage data
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProjectUsageDaily {
    pub avg_cpu_utilised: f32,
    pub avg_mem_utilised: f32,
    pub billable_vcpu_hours: f32,
    pub build_minutes: u32,
    pub isodate: chrono::NaiveDate,
    pub max_cpu_reserved: f32,
    pub max_mem_reserved: f32,
    pub min_cpu_reserved: f32,
    pub min_mem_reserved: f32,
    pub reserved_vcpu_hours: f32,
    pub runtime_minutes: u32,
}
