use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ProvisionResourceRequest {
    /// The type of this resource
    pub r#type: ResourceType,
    /// The config used when creating this resource.
    /// Use `Self::r#type` to know how to parse this data.
    pub config: Value,
}

/// Helper for deserializing
#[derive(Deserialize)]
#[serde(untagged)] // Try deserializing as a Shuttle resource, fall back to a custom value
pub enum ResourceInput {
    Shuttle(ProvisionResourceRequest),
    Custom(Value),
}

/// The resource state represents the stage of the provisioning process the resource is in.
#[derive(
    Debug, Clone, PartialEq, Eq, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum ResourceState {
    Authorizing,
    Provisioning,
    Failed,
    Ready,
    Deleting,
    Deleted,

    /// Forward compatibility
    #[cfg(feature = "unknown-variants")]
    #[doc(hidden)]
    #[typeshare(skip)]
    #[serde(untagged, skip_serializing)]
    #[strum(default, to_string = "Unknown: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ResourceResponse {
    pub r#type: ResourceType,
    pub state: ResourceState,
    /// The config used when creating this resource. Use the `r#type` to know how to parse this data.
    pub config: Value,
    /// The output type for this resource, if state is Ready. Use the `r#type` to know how to parse this data.
    pub output: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct ResourceListResponse {
    pub resources: Vec<ResourceResponse>,
}

#[derive(
    Clone,
    Debug,
    Deserialize,
    Serialize,
    Eq,
    PartialEq,
    strum::AsRefStr,
    strum::Display,
    strum::EnumString,
)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
// is a flat enum instead of nested enum to allow typeshare
pub enum ResourceType {
    #[strum(to_string = "database::shared::postgres")]
    #[serde(rename = "database::shared::postgres")]
    DatabaseSharedPostgres,
    #[strum(to_string = "database::aws_rds::postgres")]
    #[serde(rename = "database::aws_rds::postgres")]
    DatabaseAwsRdsPostgres,
    #[strum(to_string = "database::aws_rds::mysql")]
    #[serde(rename = "database::aws_rds::mysql")]
    DatabaseAwsRdsMySql,
    #[strum(to_string = "database::aws_rds::mariadb")]
    #[serde(rename = "database::aws_rds::mariadb")]
    DatabaseAwsRdsMariaDB,
    /// (Will probably be removed)
    #[strum(to_string = "secrets")]
    #[serde(rename = "secrets")]
    Secrets,
    /// Local provisioner only
    #[strum(to_string = "container")]
    #[serde(rename = "container")]
    Container,

    /// Forward compatibility
    #[cfg(feature = "unknown-variants")]
    #[doc(hidden)]
    #[typeshare(skip)]
    #[serde(untagged, skip_serializing)]
    #[strum(default, to_string = "Unknown: {0}")]
    Unknown(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct SetEnvVarsRequest {
    vars: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn to_string_and_back() {
        let inputs = [
            ResourceType::DatabaseSharedPostgres,
            ResourceType::Secrets,
            ResourceType::Container,
        ];

        for input in inputs {
            let actual = ResourceType::from_str(input.as_ref()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
