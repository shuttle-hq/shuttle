use serde::{Deserialize, Serialize};

/// Used by the runner service to send requests to control plane, where the requested resources
/// will be provisioned.
#[derive(Serialize, Deserialize, Clone)]
pub struct ResourceRequest {
    /// The resource input returned from the runtime::load call.
    pub resources: Vec<Vec<u8>>,
}
