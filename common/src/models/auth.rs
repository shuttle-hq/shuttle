use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct UserResponse {
    pub name: String,
    pub key: String,
    pub account_tier: String,
}
