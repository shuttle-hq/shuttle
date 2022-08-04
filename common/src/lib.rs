pub mod database;
pub mod deployment;
pub mod log;
pub mod project;
pub mod resource;
pub mod service;

use std::fmt::{Display, Formatter};

use ::log::Level;
use resource::ResourceInfo;
use rocket::Responder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

// TODO: Determine error handling strategy - error types or just use `anyhow`?
#[derive(Debug, Clone, Serialize, Deserialize, Responder)]
#[response(content_type = "json")]
pub enum DeploymentApiError {
    #[response(status = 500)]
    Internal(String),
    #[response(status = 503)]
    Unavailable(String),
    #[response(status = 404)]
    NotFound(String),
    #[response(status = 400)]
    BadRequest(String),
    #[response(status = 409)]
    ProjectAlreadyExists(String),
}

impl Display for DeploymentApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentApiError::Internal(s) => write!(f, "internal: {}", s),
            DeploymentApiError::Unavailable(s) => write!(f, "unavailable: {}", s),
            DeploymentApiError::NotFound(s) => write!(f, "not found: {}", s),
            DeploymentApiError::BadRequest(s) => write!(f, "bad request: {}", s),
            DeploymentApiError::ProjectAlreadyExists(s) => write!(f, "conflict: {}", s),
        }
    }
}

impl std::error::Error for DeploymentApiError {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogItem {
    pub body: String,
    pub level: Level,
    pub target: String,
}
