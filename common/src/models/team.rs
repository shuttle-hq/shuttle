use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct TeamListResponse {
    pub teams: Vec<TeamResponse>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct TeamResponse {
    pub id: String,
    /// Display name
    pub name: String,
    /// Membership info of the calling user
    pub membership: TeamMembership,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct TeamMembersResponse {
    pub members: Vec<TeamMembership>,
    pub invites: Vec<TeamInvite>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct TeamMembership {
    pub user_id: String,
    /// Role of the user in the team
    pub role: TeamRole,
    /// Auth0 display name
    pub nickname: Option<String>,
    /// URL to profile picture
    pub picture: Option<String>,
    /// Auth0 primary email
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct TeamInvite {
    pub id: String,
    pub email: String,
    /// Role of the user in the team
    pub role: TeamRole,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[typeshare::typeshare]
pub enum TeamRole {
    Owner,
    Admin,
    Member,
}

/// Provide user id to add user.
/// Provide email address to invite user via email.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct AddTeamMemberRequest {
    pub user_id: Option<String>,
    pub email: Option<String>,
    /// Role of the user in the team
    pub role: Option<TeamRole>,
}
