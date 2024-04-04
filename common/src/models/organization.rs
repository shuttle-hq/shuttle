use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub id: String,
    pub display_name: String,
    pub is_admin: bool,
}
