use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgArgumentBuffer, PgValueRef},
    Postgres,
};

/// Used by the runner service to send requests to control plane, where the requested resources
/// will be provisioned.
#[derive(Serialize, Deserialize)]
pub struct ResourceRequest {
    /// The resource input returned from the runtime::load call.
    pub resources: Vec<Vec<u8>>,
}

/// The resource state represents the stage of the provisioning process the resource is in.
#[derive(
    Debug, Clone, PartialEq, Eq, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[strum(serialize_all = "lowercase")]
pub enum ResourceState {
    Authorizing,
    Provisioning,
    Failed,
    Ready,
    Deleting,
    Deleted,
}

impl<DB: sqlx::Database> sqlx::Type<DB> for ResourceState
where
    str: sqlx::Type<DB>,
{
    fn type_info() -> <DB as sqlx::Database>::TypeInfo {
        <str as sqlx::Type<DB>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ResourceState {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> sqlx::encode::IsNull {
        <&str as sqlx::Encode<Postgres>>::encode(&self.to_string(), buf)
    }
}

impl<'r> sqlx::Decode<'r, Postgres> for ResourceState {
    fn decode(value: PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <&str as sqlx::Decode<Postgres>>::decode(value)?;

        let state = ResourceState::from_str(value)?;
        Ok(state)
    }
}

/// Used by the runner service to send requests to control plane, where the requested resources
/// will be provisioned.
#[derive(Serialize, Deserialize)]
pub struct ResourceResponse {
    /// The resource output returned from the control plane after provisioning.
    pub resources: Vec<Vec<u8>>,
    /// The state of the resource.
    pub state: ResourceState,
}
