use std::{collections::HashMap, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[cfg(feature = "display")]
use crossterm::style::Stylize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Display, Serialize, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[strum(ascii_case_insensitive)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum DeploymentState {
    Pending,
    Building,
    Running,
    #[strum(serialize = "in progress")]
    InProgress,
    Stopped,
    Stopping,
    Failed,

    /// Forward compatibility
    #[cfg(feature = "unknown-variants")]
    #[doc(hidden)]
    #[typeshare(skip)]
    #[serde(untagged, skip_serializing)]
    #[strum(default, to_string = "Unknown: {0}")]
    Unknown(String),
}

impl DeploymentState {
    #[cfg(feature = "display")]
    pub fn get_color_crossterm(&self) -> crossterm::style::Color {
        use crossterm::style::Color;

        match self {
            Self::Pending => Color::DarkYellow,
            Self::Building => Color::Yellow,
            Self::InProgress => Color::Cyan,
            Self::Running => Color::Green,
            Self::Stopped => Color::DarkBlue,
            Self::Stopping => Color::Blue,
            Self::Failed => Color::Red,
            #[cfg(feature = "unknown-variants")]
            Self::Unknown(_) => Color::Grey,
        }
    }
    #[cfg(all(feature = "tables", feature = "display"))]
    pub fn get_color_comfy_table(&self) -> comfy_table::Color {
        use comfy_table::Color;

        match self {
            Self::Pending => Color::DarkYellow,
            Self::Building => Color::Yellow,
            Self::InProgress => Color::Cyan,
            Self::Running => Color::Green,
            Self::Stopped => Color::DarkBlue,
            Self::Stopping => Color::Blue,
            Self::Failed => Color::Red,
            #[cfg(feature = "unknown-variants")]
            Self::Unknown(_) => Color::Grey,
        }
    }
    #[cfg(feature = "display")]
    pub fn to_string_colored(&self) -> String {
        self.to_string()
            .with(self.get_color_crossterm())
            .to_string()
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct DeploymentListResponse {
    pub deployments: Vec<DeploymentResponse>,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct DeploymentResponse {
    pub id: String,
    pub state: DeploymentState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// URIs where this deployment can currently be reached (only relevant for Running state)
    pub uris: Vec<String>,
    pub build_id: Option<String>,
    pub build_meta: Option<BuildMeta>,
}

#[cfg(feature = "display")]
impl DeploymentResponse {
    pub fn to_string_summary_colored(&self) -> String {
        // TODO: make this look nicer
        format!(
            "Deployment {} - {}",
            self.id.as_str().bold(),
            self.state.to_string_colored(),
        )
    }
    pub fn to_string_colored(&self) -> String {
        // TODO: make this look nicer
        format!(
            "Deployment {} - {}\n{}",
            self.id.as_str().bold(),
            self.state.to_string_colored(),
            self.uris.join("\n"),
        )
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct UploadArchiveResponse {
    /// The S3 object version ID of the uploaded object
    pub archive_version_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "content")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum DeploymentRequest {
    /// Build an image from the source code in an attached zip archive
    BuildArchive(DeploymentRequestBuildArchive),
    // TODO?: Add GitRepo(DeploymentRequestGitRepo)
    /// Use this image directly. Can be used to skip the build step.
    Image(DeploymentRequestImage),
    //
    // No Unknown variant: is a Request type and should only be deserialized on backend
}

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct DeploymentRequestBuildArchive {
    /// The S3 object version ID of the archive to use
    pub archive_version_id: String,
    pub build_args: Option<BuildArgs>,
    /// Secrets to add before this deployment.
    /// TODO: Remove this in favour of a separate secrets uploading action.
    pub secrets: Option<HashMap<String, String>>,
    pub build_meta: Option<BuildMeta>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "content")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum BuildArgs {
    Rust(BuildArgsRust),
    //
    // No Unknown variant: is a Request type and should only be deserialized on backend
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct BuildArgsRust {
    /// Version of shuttle-runtime used by this crate
    pub shuttle_runtime_version: Option<String>,
    /// Use the built in cargo chef setup for caching
    pub cargo_chef: bool,
    /// Build with the built in `cargo build` setup
    pub cargo_build: bool,
    /// The cargo package name to compile
    pub package_name: Option<String>,
    /// The cargo binary name to compile
    pub binary_name: Option<String>,
    /// comma-separated list of features to activate
    pub features: Option<String>,
    /// Passed on to `cargo build`
    pub no_default_features: bool,
    /// Use the mold linker
    pub mold: bool,
}

impl Default for BuildArgsRust {
    fn default() -> Self {
        Self {
            shuttle_runtime_version: Default::default(),
            cargo_chef: true,
            cargo_build: true,
            package_name: Default::default(),
            binary_name: Default::default(),
            features: Default::default(),
            no_default_features: Default::default(),
            mold: Default::default(),
        }
    }
}

/// Max length of strings in the git metadata
pub const GIT_STRINGS_MAX_LENGTH: usize = 80;

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct BuildMeta {
    pub git_commit_id: Option<String>,
    pub git_commit_msg: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

#[cfg(feature = "display")]
impl std::fmt::Display for BuildMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(true) = self.git_dirty {
            write!(f, "(dirty) ")?;
        }
        if let Some(ref c) = self.git_commit_id {
            write!(f, "[{}] ", c.chars().take(7).collect::<String>())?;
        }
        if let Some(ref m) = self.git_commit_msg {
            write!(f, "{m}")?;
        }

        Ok(())
    }
}

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct DeploymentRequestImage {
    pub image: String,
    /// TODO: Remove this in favour of a separate secrets uploading action.
    pub secrets: Option<HashMap<String, String>>,
    // TODO: credentials fields for private repos??
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
    //
    // No Unknown variant: is not deserialized in user facing libraries (just FromStr parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn deployment_state_from_and_to_str() {
        assert_eq!(
            DeploymentState::Building,
            DeploymentState::from_str("Building").unwrap()
        );
        assert_eq!(
            DeploymentState::Building,
            DeploymentState::from_str("BuilDing").unwrap()
        );
        assert_eq!(
            DeploymentState::Building,
            DeploymentState::from_str("building").unwrap()
        );
        assert_eq!(
            DeploymentState::Building.to_string(),
            "building".to_string()
        );
    }

    #[cfg(feature = "unknown-variants")]
    #[test]
    fn unknown_state() {
        assert_eq!(
            DeploymentState::Unknown("flying".to_string()),
            DeploymentState::from_str("flying").unwrap()
        );
        assert_eq!(
            DeploymentState::Unknown("flying".to_string()).to_string(),
            "Unknown: flying".to_string()
        );
    }

    #[test]
    fn env_from_str() {
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
