use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub name: String,
    pub key: String,
    pub projects: Vec<String>,
}
