use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::database;

/// Common type to hold all the information we need for a generic resource
#[derive(Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::resource::Response))]
pub struct Response {
    /// The type of this resource.
    #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::resource::Type))]
    pub r#type: Type,

    /// The config used when creating this resource. Use the [Self::r#type] to know how to parse this data.
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub config: Value,

    /// The data associated with this resource. Use the [Self::r#type] to know how to parse this data.
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::resource::Type))]
pub enum Type {
    #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::database::Type))]
    Database(database::Type),
    Secrets,
    StaticFolder,
    Persist,
}

impl Response {
    pub fn into_bytes(self) -> Vec<u8> {
        self.to_bytes()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("to turn resource into a vec")
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        serde_json::from_slice(&bytes).expect("to turn bytes into a resource")
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
            Type::Secrets => write!(f, "secrets"),
            Type::StaticFolder => write!(f, "static_folder"),
            Type::Persist => write!(f, "persist"),
        }
    }
}
