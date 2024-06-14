use serde::{Deserialize, Serialize};

/// Used by the control plane to return provisioned resource data to the runner.
#[derive(Serialize, Deserialize, Clone)]
pub struct ResourceResponse {
    /// The resource output returned from provisioning.
    pub resource: Vec<u8>,
}
