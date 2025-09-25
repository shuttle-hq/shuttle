use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GithubInstallationsResponse {
    pub accounts: Vec<GithubInstallation>,
    pub repos: Vec<GithubInstallationRepo>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GithubInstallation {
    pub installation_id: u32, // TODO: change to u64 when typeshare supports it
    pub gh_account_id: u32,   // TODO: change to u64 when typeshare supports it
    pub gh_account_name: String,
    pub gh_account_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GithubInstallationRepo {
    pub installation_id: u32, // TODO: change to u64 when typeshare supports it
    pub repo_id: u32,         // TODO: change to u64 when typeshare supports it
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GithubRepoLink {
    pub project_id: String,
    pub repo: GithubInstallationRepo,
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct LinkGithubRepoRequest {
    pub installation_id: u32, // TODO: change to u64 when typeshare supports it
    pub repo_id: u32,         // TODO: change to u64 when typeshare supports it
    pub branch: Option<String>,
}
