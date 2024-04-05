#[derive(serde::Serialize, serde::Deserialize)]
pub struct ResourceRequest {
    pub resources: Vec<Vec<u8>>,
}
