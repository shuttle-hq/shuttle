use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::models::admin::ProjectResponse))]
pub struct ProjectResponse {
    pub project_name: String,
    pub account_name: String,
}
