use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct LoadRequest {
    pub id: Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct LoadResponse {
    pub builds_count: usize,
    pub has_capacity: bool,
}
