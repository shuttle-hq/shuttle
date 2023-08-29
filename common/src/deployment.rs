use std::{path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::project::ProjectName;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Display, Serialize, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[strum(ascii_case_insensitive)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::deployment::State))]
pub enum State {
    Queued,
    Building,
    Built,
    Loading,
    Running,
    Completed,
    Stopped,
    Crashed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMetadata {
    pub env: Environment,
    pub project_name: ProjectName,
    /// Typically your crate name
    pub service_name: String,
    /// Path to a folder that persists between deployments
    pub storage_path: PathBuf,
}

/// The environment this project is running in
#[derive(Clone, Copy, Debug, EnumString, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Environment {
    Local,
    #[strum(serialize = "production")] // Keep this around for a while for backward compat
    Deployment,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Local
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_deser() {
        assert_eq!(State::Queued, State::from_str("Queued").unwrap());
        assert_eq!(State::Unknown, State::from_str("unKnown").unwrap());
        assert_eq!(State::Built, State::from_str("built").unwrap());
    }

    #[test]
    fn test_env_deser() {
        assert_eq!(Environment::Local, Environment::from_str("local").unwrap());
        assert_eq!(
            Environment::Deployment,
            Environment::from_str("production").unwrap()
        );
        assert!(State::from_str("somewhere_else").is_err());
    }
}
