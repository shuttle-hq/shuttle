pub mod project;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use project::ProjectConfig;
use rocket::http::Status;
use rocket::Responder;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[macro_use]
extern crate lazy_static;

pub const UNVEIL_PROJECT_HEADER: &str = "Unveil-Project";

#[cfg(debug_assertions)]
lazy_static! {
    pub static ref API_URL: String =
        std::env::var("UNVEIL_API").unwrap_or_else(|_| "http://localhost:8001".to_string());
}

#[cfg(not(debug_assertions))]
lazy_static! {
    pub static ref API_URL: String = { "https://api.shuttle.rs".to_string() };
}

pub type ApiKey = String;
pub type Host = String;
pub type DeploymentId = Uuid;
pub type Port = u16;

/// Deployment metadata. This serves two purposes. Storing information
/// used for the deployment process and also providing the client with
/// information on the state of the deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMeta {
    pub id: DeploymentId,
    pub config: ProjectConfig,
    pub state: DeploymentStateMeta,
    pub host: String,
    pub build_logs: Option<String>,
    pub runtime_logs: Option<String>,
    pub database_deployment: Option<DatabaseReadyInfo>,
    pub created_at: DateTime<Utc>,
}

impl DeploymentMeta {
    pub fn queued(config: &ProjectConfig) -> Self {
        Self::new(config, DeploymentStateMeta::Queued)
    }

    pub fn built(config: &ProjectConfig) -> Self {
        Self::new(config, DeploymentStateMeta::Built)
    }

    fn new(config: &ProjectConfig, state: DeploymentStateMeta) -> Self {
        Self {
            id: Uuid::new_v4(),
            config: config.clone(),
            state,
            host: Self::create_host(config),
            build_logs: None,
            runtime_logs: None,
            database_deployment: None,
            created_at: Utc::now(),
        }
    }

    pub fn create_host(project_config: &ProjectConfig) -> Host {
        format!("{}.unveil.sh", project_config.name())
    }
}

lazy_static! {
    static ref PUBLIC_IP: String =
        std::env::var("PUBLIC_IP").unwrap_or_else(|_| "localhost".to_string());
}

impl Display for DeploymentMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let db = {
            if let Some(info) = &self.database_deployment {
                format!(
                    "\n        Database URI:       {}",
                    info.connection_string(&*PUBLIC_IP)
                )
            } else {
                "".to_string()
            }
        };
        write!(
            f,
            r#"
        Project:            {}
        Deployment Id:      {}
        Deployment Status:  {}
        Host:               {}
        Created At:         {}{}
        "#,
            self.config.name(),
            self.id,
            self.state,
            self.host,
            self.created_at,
            db
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseReadyInfo {
    pub role_name: String,
    pub role_password: String,
    pub database_name: String,
}

impl DatabaseReadyInfo {
    pub fn connection_string(&self, ip: &str) -> String {
        format!(
            "postgres://{}:{}@{}/{}",
            self.role_name, self.role_password, ip, self.database_name
        )
    }
}

/// A label used to represent the deployment state in `DeploymentMeta`
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DeploymentStateMeta {
    Queued,
    Built,
    Loaded,
    Deployed,
    Error,
}

impl Display for DeploymentStateMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DeploymentStateMeta::Queued => "QUEUED",
            DeploymentStateMeta::Built => "BUILT",
            DeploymentStateMeta::Loaded => "LOADED",
            DeploymentStateMeta::Deployed => "DEPLOYED",
            DeploymentStateMeta::Error => "ERROR",
        };
        write!(f, "{}", s)
    }
}

// TODO: Determine error handling strategy - error types or just use `anyhow`?
#[derive(Debug, Clone, Serialize, Deserialize, Responder)]
pub enum DeploymentApiError {
    #[response(status = 500)]
    Internal(String),
    #[response(status = 404)]
    NotFound(String),
    #[response(status = 400)]
    BadRequest(String),
}

impl Display for DeploymentApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentApiError::Internal(s) => write!(f, "internal: {}", s),
            DeploymentApiError::NotFound(s) => write!(f, "not found: {}", s),
            DeploymentApiError::BadRequest(s) => write!(f, "bad request: {}", s),
        }
    }
}

impl std::error::Error for DeploymentApiError {}
