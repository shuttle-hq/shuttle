use serde::{Deserialize, Serialize};

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
