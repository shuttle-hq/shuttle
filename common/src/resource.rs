use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::database;

/// Common type to hold all the information we need for a generic resource
#[derive(Clone, Deserialize, Serialize)]
pub struct Response {
    /// The type of this resource.
    pub r#type: Type,

    /// The config used when creating this resource. Use the [Self::r#type] to know how to parse this data.
    pub config: Value,

    /// The data associated with this resource. Use the [Self::r#type] to know how to parse this data.
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Database(database::Type),
    Secrets,
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
        }
    }
}
