#[cfg(feature = "backend")]
pub mod backends;
#[cfg(feature = "claims")]
pub mod claims;
#[cfg(feature = "service")]
pub mod database;
#[cfg(feature = "service")]
pub mod deployment;
#[cfg(feature = "service")]
pub mod log;
#[cfg(feature = "models")]
pub mod models;
#[cfg(feature = "service")]
pub mod project;
pub mod resource;
#[cfg(feature = "service")]
pub mod storage_manager;
#[cfg(feature = "tracing")]
pub mod tracing;
#[cfg(feature = "wasm")]
pub mod wasm;

use serde::{Deserialize, Serialize};
#[cfg(feature = "service")]
use uuid::Uuid;

#[cfg(feature = "service")]
pub use log::Item as LogItem;
#[cfg(feature = "service")]
pub use log::STATE_MESSAGE;

#[cfg(debug_assertions)]
pub const API_URL_DEFAULT: &str = "http://localhost:8001";

#[cfg(not(debug_assertions))]
pub const API_URL_DEFAULT: &str = "https://api.shuttle.rs";

pub type ApiKey = String;
pub type ApiUrl = String;
pub type Host = String;
#[cfg(feature = "service")]
pub type DeploymentId = Uuid;

#[cfg(feature = "error")]
/// Errors that can occur when changing types. Especially from prost
#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("failed to parse timestamp: {0}")]
    Timestamp(#[from] prost_types::TimestampError),
    #[error("failed to parse serde: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseReadyInfo {
    engine: String,
    role_name: String,
    role_password: String,
    database_name: String,
    port: String,
    address_private: String,
    address_public: String,
}

impl DatabaseReadyInfo {
    pub fn new(
        engine: String,
        role_name: String,
        role_password: String,
        database_name: String,
        port: String,
        address_private: String,
        address_public: String,
    ) -> Self {
        Self {
            engine,
            role_name,
            role_password,
            database_name,
            port,
            address_private,
            address_public,
        }
    }
    pub fn connection_string_private(&self) -> String {
        format!(
            "{}://{}:{}@{}:{}/{}",
            self.engine,
            self.role_name,
            self.role_password,
            self.address_private,
            self.port,
            self.database_name
        )
    }
    pub fn connection_string_public(&self) -> String {
        format!(
            "{}://{}:{}@{}:{}/{}",
            self.engine,
            self.role_name,
            self.role_password,
            self.address_public,
            self.port,
            self.database_name
        )
    }
}
