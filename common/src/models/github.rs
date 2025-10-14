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
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct UpdateGithubRepoBranchRequest {
    pub branch: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GithubInstallationGenerateRepoRequest {
    pub installation_id: u32, // TODO: change to u64 when typeshare supports it
    pub template_owner: String,
    pub template_name: String,
    pub owner: String,
    pub repo_name: String,
    pub description: Option<String>,
    pub include_all_branches: Option<bool>,
    pub private: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct CreateDeploymentFromGithubRequest {
    pub branch: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GetGithubRepoBranchesResponse {
    pub branches: Vec<GithubBranch>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GithubBranch {
    pub name: String,
    pub protected: bool,
}

/// Internal
#[derive(Deserialize, Serialize)]
pub struct GithubDeployerDeployRequest {
    pub personal_token: String,
    pub owner: String,
    pub repo: String,
    pub commit_ref: String,
    pub commit_msg: Option<String>,
    pub branch: String,
    pub project_id: String,
}
