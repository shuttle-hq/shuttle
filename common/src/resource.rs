use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{database, DatabaseReadyInfo};

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub service_id: Uuid,
    pub r#type: Type,
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Database(database::Type),
}

/// Trait used to get information from all the resources we manage
pub trait ResourceInfo {
    /// String to connect to this resource from a public location
    fn connection_string_public(&self) -> String;
}

impl Response {
    pub fn get_resource_info(&self) -> impl ResourceInfo {
        match self.r#type {
            Type::Database(_) => {
                serde_json::from_value::<DatabaseReadyInfo>(self.data.clone()).unwrap()
            }
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
        }
    }
}
