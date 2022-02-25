use rocket::http::Status;
use rocket::Request;
use rocket::request::{FromRequest, Outcome};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const UNVEIL_PROJECT_HEADER: &'static str = "Unveil-Project";

pub type DeploymentId = Uuid;

/// Deployment metadata. This serves two purposes. Storing information
/// used for the deployment process and also providing the client with
/// information on the state of the deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMeta {
    pub id: DeploymentId,
    pub config: ProjectConfig,
    pub state: DeploymentStateMeta,
    pub url: String,
    pub build_logs: Option<String>,
    pub runtime_logs: Option<String>,
}


impl DeploymentMeta {
    pub fn new(config: &ProjectConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config: config.clone(),
            state: DeploymentStateMeta::QUEUED,
            url: Self::create_url(config),
            build_logs: None,
            runtime_logs: None,
        }
    }

    pub fn create_url(project_config: &ProjectConfig) -> String {
        format!("{}.unveil.sh", project_config.name)
    }
}

/// A label used to represent the deployment state in `DeploymentMeta`
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DeploymentStateMeta {
    QUEUED,
    BUILT,
    LOADED,
    DEPLOYED,
    ERROR,
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
            Some(config_string) => config_string
        };

        match serde_json::from_str::<ProjectConfig>(config_string) {
            Ok(config) => Outcome::Success(config),
            Err(_) => Outcome::Failure((Status::BadRequest, ProjectConfigError::Malformed))
        }
    }
}
