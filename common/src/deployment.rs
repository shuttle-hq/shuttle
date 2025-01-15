use std::{path::PathBuf, str::FromStr};

use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Display, Serialize, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[strum(ascii_case_insensitive)]
#[typeshare::typeshare]
pub enum DeploymentStateBeta {
    Pending,
    Building,
    Running,
    #[strum(serialize = "in progress")]
    InProgress,
    Stopped,
    Stopping,
    Failed,
    /// Fallback
    Unknown,
}

impl DeploymentStateBeta {
    /// We return a &str rather than a Color here, since `comfy-table` re-exports
    /// crossterm::style::Color and we depend on both `comfy-table` and `crossterm`
    /// we may end up with two different versions of Color.
    pub fn get_color(&self) -> &str {
        match self {
            Self::Pending => "dark_yellow",
            Self::Building => "yellow",
            Self::InProgress => "cyan",
            Self::Running => "green",
            Self::Stopped => "dark_blue",
            Self::Stopping => "blue",
            Self::Failed => "red",
            Self::Unknown => "grey",
        }
    }
    pub fn to_string_colored(&self) -> String {
        // Unwrap is safe because Color::from_str returns the color white if the argument is not a Color.
        self.to_string()
            .with(crossterm::style::Color::from_str(self.get_color()).unwrap())
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMetadata {
    pub env: Environment,
    pub project_name: String,
    /// Path to a folder that persists between deployments
    pub storage_path: PathBuf,
}

/// The environment this project is running in
#[derive(
    Clone, Copy, Debug, Default, Display, EnumString, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Environment {
    #[default]
    Local,
    #[strum(serialize = "production")] // Keep this around for a while for backward compat
    Deployment,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_state_deser() {
        assert_eq!(
            DeploymentStateBeta::Building,
            DeploymentStateBeta::from_str("Building").unwrap()
        );
        assert_eq!(
            DeploymentStateBeta::Building,
            DeploymentStateBeta::from_str("BuilDing").unwrap()
        );
        assert_eq!(
            DeploymentStateBeta::Building,
            DeploymentStateBeta::from_str("building").unwrap()
        );
    }

    #[test]
    fn test_env_deser() {
        assert_eq!(Environment::Local, Environment::from_str("local").unwrap());
        assert_eq!(
            Environment::Deployment,
            Environment::from_str("production").unwrap()
        );
        assert!(Environment::from_str("somewhere_else").is_err());
        assert_eq!(format!("{:?}", Environment::Local), "Local".to_owned());
        assert_eq!(format!("{}", Environment::Local), "local".to_owned());
        assert_eq!(Environment::Local.to_string(), "local".to_owned());
    }
}
