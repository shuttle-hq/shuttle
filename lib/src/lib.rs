use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use rocket::{Request, Responder};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub const UNVEIL_PROJECT_HEADER: &'static str = "Unveil-Project";

#[cfg(debug_assertions)]
pub const API_URL: &'static str = "http://localhost:8001";

#[cfg(not(debug_assertions))]
pub const API_URL: &'static str = "https://21ac7btou0.execute-api.eu-west-2.amazonaws.com/valpha";

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
    pub created_at: DateTime<Utc>,
}

impl DeploymentMeta {
    pub fn new(config: &ProjectConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config: config.clone(),
            state: DeploymentStateMeta::Queued,
            host: Self::create_host(config),
            build_logs: None,
            runtime_logs: None,
            created_at: Utc::now()
        }
    }

    pub fn create_host(project_config: &ProjectConfig) -> Host {
        format!("{}.unveil.sh", project_config.name)
    }
}

impl Display for DeploymentMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
        Project:            {}
        Deployment Id:      {}
        Deployment Status:  {}
        Host:               {}
        Created At:         {}
        "#,
            self.config.name, self.id, self.state, self.host, self.created_at
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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectConfig {
    pub name: String,
}

#[derive(Debug)]
pub enum ProjectConfigError {
    Missing,
    Malformed,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ProjectConfig {
    type Error = ProjectConfigError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config_string = match req.headers().get_one(UNVEIL_PROJECT_HEADER) {
            None => return Outcome::Failure((Status::BadRequest, ProjectConfigError::Missing)),
            Some(config_string) => config_string,
        };

        match serde_json::from_str::<ProjectConfig>(config_string) {
            Ok(config) => Outcome::Success(config),
            Err(_) => Outcome::Failure((Status::BadRequest, ProjectConfigError::Malformed)),
        }
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

impl std::error::Error for DeploymentApiError {

}
