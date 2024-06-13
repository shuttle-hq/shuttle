use serde::{Deserialize, Serialize};

/// Used by the runner service to send requests to control plane, where the requested resources
/// will be provisioned.
#[derive(Serialize, Deserialize, Clone)]
pub struct LoadResponse {
    /// The resource input returned from the runtime::load call.
    pub resources: Vec<Vec<u8>>,
}

/// Used by the control plane to return provisioned resource data to the runner.
#[derive(Serialize, Deserialize, Clone)]
pub struct ResourceResponse {
    /// The resource output returned from provisioning.
    pub resource: Vec<u8>,
}
