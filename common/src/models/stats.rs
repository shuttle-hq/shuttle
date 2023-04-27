use serde::{Deserialize, Serialize};
#[cfg(feature = "openapi")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::models::stats::LoadRequest))]
pub struct LoadRequest {
    pub id: Uuid,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::models::stats::LoadResponse))]
pub struct LoadResponse {
    pub builds_count: usize,
    pub has_capacity: bool,
}
