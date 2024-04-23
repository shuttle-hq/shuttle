use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::user::UserId;

/// Minimal team information
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Response {
    /// Team ID
    pub id: String,

    /// Name used for display purposes
    pub display_name: String,

    /// Is this user an admin of the team
    pub is_admin: bool,
}

/// Member of a team
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MemberResponse {
    /// User ID
    pub id: UserId,

    /// Role of the user in the team
    pub role: MemberRole,
}

/// Role of a user in a team
#[derive(Debug, Serialize, Deserialize, PartialEq, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MemberRole {
    Admin,
    Member,
}
