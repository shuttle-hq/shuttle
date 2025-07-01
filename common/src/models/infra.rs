use serde::{Deserialize, Serialize};

use super::project::ComputeTier;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct InfraRequest {
    pub instance_size: Option<ComputeTier>,
    // pub replicas: Option<u8>,
}
