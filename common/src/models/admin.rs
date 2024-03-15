use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ProjectResponse {
    pub project_name: String,
    pub account_name: String,
    pub user_id: String,
}
