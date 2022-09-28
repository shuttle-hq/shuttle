pub mod database;
#[cfg(feature = "full")]
pub mod deployment;
#[cfg(feature = "full")]
pub mod log;
pub mod project;
pub mod resource;
#[cfg(feature = "full")]
pub mod secret;
#[cfg(feature = "full")]
pub mod service;
pub mod user;

#[cfg(feature = "full")]
pub mod version;

#[cfg(feature = "full")]
pub use log::Item as LogItem;

use resource::ResourceInfo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "full")]
pub use crate::log::STATE_MESSAGE;

#[cfg(debug_assertions)]
pub const API_URL_DEFAULT: &str = "http://localhost:8001";

#[cfg(not(debug_assertions))]
pub const API_URL_DEFAULT: &str = "https://api.shuttle.rs";

pub type ApiKey = String;
pub type ApiUrl = String;
pub type Host = String;
pub type DeploymentId = Uuid;
pub type Port = u16;

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

impl ResourceInfo for DatabaseReadyInfo {
    fn connection_string_public(&self) -> String {
        self.connection_string_public()
    }
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
