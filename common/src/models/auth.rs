use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct TokenMessage {
    /// The token of this sign in request
    pub token: String,
    /// The Console URL where this token can be authorized
    pub url: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct KeyMessage {
    pub api_key: String,
}
