use std::collections::BTreeMap;

/// API model for project creation requests.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq)]
pub struct CreateProjectRequest {
    pub name: String,
    pub kind: ProjectKind,
}

/// API model for deployment creation requests.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq)]
pub struct CreateDeploymentRequest {
    pub kind: ProjectKind,
    pub resources: Vec<ShuttleResource>,
    /// Additional configuration information
    #[serde(flatten, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// API model for project status responses.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ProjectStatusResponse {
    pub id: String,
    pub name: String,
    pub kind: ProjectKind,
    pub resources: Vec<ShuttleResource>,
    pub url: Option<String>,
    // TODO: for the env vars we assume that there is only one container, which is true at the time
    // of writing, but will need to be adapted when we support deploying multiple containers.
    pub env: Option<BTreeMap<String, String>>,
    pub condition: AggregateProjectCondition,
}

/// Aggregated condition information for a project including its resources, workload, and overall state
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AggregateProjectCondition {
    pub resources: ResourcesState,
    pub workload: WorkloadState,
    pub project: ProjectState,
}

/// The current deployment state of a project
#[derive(
    Eq, Copy, Hash, Clone, Debug, PartialEq, strum::Display, serde::Serialize, serde::Deserialize,
)]
pub enum ProjectState {
    /// No project status available
    Empty,
    /// The project is deployed and available
    Available,
    /// Created but not yet deployed
    Created,
}
/// The current state of a project's workload
#[derive(
    Eq, Hash, Clone, Debug, PartialEq, strum::Display, serde::Serialize, serde::Deserialize,
)]
pub enum WorkloadState {
    /// No workload status available
    Empty,
    /// The workload is failing with the specified error message
    Failing(String),
    /// The workload is currently deploying
    Deploying,
    /// The workload is running successfully
    Running,
    /// The workload state is unknown
    Unknown,
}
