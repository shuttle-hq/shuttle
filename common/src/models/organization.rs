use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::user::UserId;

/// Minimal organization information
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Response {
    /// Organization ID
    pub id: String,

    /// Name used for display purposes
    pub display_name: String,

    /// Is this user an admin of the organization
    pub is_admin: bool,
}

/// Member of an organization
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MemberResponse {
    /// User ID
    pub id: UserId,

    /// Role of the user in the organization
    pub role: MemberRole,
}

/// Role of a user in an organization
#[derive(Debug, Serialize, Deserialize, PartialEq, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MemberRole {
    Admin,
    Member,
}
