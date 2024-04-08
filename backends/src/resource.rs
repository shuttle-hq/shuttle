use serde::{Deserialize, Serialize};

/// Used by the runner service to send requests to control plane, where the requested resources
/// will be provisioned.
#[derive(Serialize, Deserialize)]
pub struct ResourceRequest {
    /// The resource input returned from the runtime::load call.
    pub resources: Vec<Vec<u8>>,
}

/// Used to request a shared DB from the provisioner service.
#[derive(Deserialize, Serialize)]
pub struct SharedDbRequest {
    pub project_id: String,
}

/// Used to request the deletion of a shared DB from the provisioner service.
#[derive(Deserialize, Serialize)]
pub struct SharedDbDeletionRequest {
    pub db_name: String,
    pub role_name: String,
}
