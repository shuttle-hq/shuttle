pub mod database;
pub mod project;

use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
};

use chrono::{DateTime, Utc};
use log::Level;
use rocket::Responder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::ProjectName;

pub const SHUTTLE_PROJECT_HEADER: &str = "Shuttle-Project";

#[cfg(debug_assertions)]
pub const API_URL_DEFAULT: &str = "http://localhost:8001";

#[cfg(not(debug_assertions))]
pub const API_URL_DEFAULT: &str = "https://api.shuttle.rs";

pub type ApiKey = String;
pub type ApiUrl = String;
pub type Host = String;
pub type DeploymentId = Uuid;
pub type Port = u16;

/// Deployment metadata. This serves two purposes. Storing information
/// used for the deployment process and also providing the client with
/// information on the state of the deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMeta {
    pub id: DeploymentId,
    pub project: ProjectName,
    pub state: DeploymentStateMeta,
    pub host: String,
    pub build_logs: Option<String>,
    pub runtime_logs: BTreeMap<DateTime<Utc>, LogItem>,
    pub database_deployment: Option<DatabaseReadyInfo>,
    pub created_at: DateTime<Utc>,
}

impl DeploymentMeta {
    pub fn queued(fqdn: &str, project: ProjectName) -> Self {
        Self::new(fqdn, project, DeploymentStateMeta::Queued)
    }

    pub fn built(fqdn: &str, project: ProjectName) -> Self {
        Self::new(fqdn, project, DeploymentStateMeta::Built)
    }

    fn new(fqdn: &str, project: ProjectName, state: DeploymentStateMeta) -> Self {
        let host = Self::create_host(fqdn, &project);
        Self {
            id: Uuid::new_v4(),
            project,
            state,
            host,
            build_logs: None,
            runtime_logs: BTreeMap::new(),
            database_deployment: None,
            created_at: Utc::now(),
        }
    }

    pub fn create_host(fqdn: &str, project_name: &ProjectName) -> Host {
        format!("{}.{}", project_name, fqdn)
    }
}

impl Display for DeploymentMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let db = {
            if let Some(info) = &self.database_deployment {
                format!(
                    "\n        Database URI:       {}",
                    info.connection_string_public()
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
        Host:               https://{}
        Created At:         {}{}
        "#,
            self.project, self.id, self.state, self.host, self.created_at, db
        )
    }
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

/// A label used to represent the deployment state in `DeploymentMeta`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStateMeta {
    Queued,
    Built,
    Loaded,
    Deployed,
    Error(String),
    Deleted,
}

impl Display for DeploymentStateMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DeploymentStateMeta::Queued => "QUEUED".to_string(),
            DeploymentStateMeta::Built => "BUILT".to_string(),
            DeploymentStateMeta::Loaded => "LOADED".to_string(),
            DeploymentStateMeta::Deployed => "DEPLOYED".to_string(),
            DeploymentStateMeta::Error(msg) => format!("ERROR: {}", &msg),
            DeploymentStateMeta::Deleted => "DELETED".to_string(),
        };
        write!(f, "{}", s)
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

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct BuildLog {
    pub id: Uuid,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}
