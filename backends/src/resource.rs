use serde::{Deserialize, Serialize};
use shuttle_common::database::AwsRdsEngine;

/// Used by the runner service to send requests to control plane, where the requested resources
/// will be provisioned.
#[derive(Serialize, Deserialize)]
pub struct ResourceRequest {
    /// The resource input returned from the runtime::load call.
    pub resources: Vec<Vec<u8>>,
}

/// Used to request the provisioning or deletion of a shared DB from the provisioner service.
#[derive(Deserialize, Serialize)]
pub struct SharedDbRequest {
    pub db_name: String,
    pub role_name: String,
}

/// Used to request the provisioning or deletion of an AWS RDS instance from the provisioner
/// service.
#[derive(Deserialize, Serialize)]
pub struct RdsRequest {
    pub database_engine: AwsRdsEngine,
    /// Extracted from the resource config if it exists.
    pub database_name: Option<String>,
    pub project_id: String,
}
