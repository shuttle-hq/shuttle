use std::{collections::HashMap, path::PathBuf, str::FromStr};

use chrono::{DateTime, Local, SecondsFormat, Utc};
use comfy_table::{
    presets::{NOTHING, UTF8_BORDERS_ONLY},
    Attribute, Cell, Color, ContentArrangement, Table,
};
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

#[derive(Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct DeploymentListResponseBeta {
    pub deployments: Vec<DeploymentResponseBeta>,
}

#[derive(Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct DeploymentResponseBeta {
    pub id: String,
    pub state: DeploymentStateBeta,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// URIs where this deployment can currently be reached (only relevant for Running state)
    pub uris: Vec<String>,
    pub build_id: Option<String>,
    pub build_meta: Option<BuildMetaBeta>,
}

impl DeploymentResponseBeta {
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

pub fn deployments_table_beta(deployments: &[DeploymentResponseBeta], raw: bool) -> String {
    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Deployment ID", "Status", "Date", "Git revision"]);

    for deploy in deployments.iter() {
        let datetime: DateTime<Local> = DateTime::from(deploy.created_at);
        table.add_row(vec![
            Cell::new(&deploy.id).add_attribute(Attribute::Bold),
            Cell::new(&deploy.state)
                // Unwrap is safe because Color::from_str returns the color white if str is not a Color.
                .fg(Color::from_str(deploy.state.get_color()).unwrap()),
            Cell::new(datetime.to_rfc3339_opts(SecondsFormat::Secs, false)),
            Cell::new(
                deploy
                    .build_meta
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ),
        ]);
    }

    table.to_string()
}

#[derive(Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct UploadArchiveResponseBeta {
    /// The S3 object version ID of the uploaded object
    pub archive_version_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "content")]
#[typeshare::typeshare]
pub enum DeploymentRequestBeta {
    /// Build an image from the source code in an attached zip archive
    BuildArchive(DeploymentRequestBuildArchiveBeta),
    // TODO?: Add GitRepo(DeploymentRequestGitRepoBeta)
    /// Use this image directly. Can be used to skip the build step.
    Image(DeploymentRequestImageBeta),
}

#[derive(Default, Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct DeploymentRequestBuildArchiveBeta {
    /// The S3 object version ID of the archive to use
    pub archive_version_id: String,
    pub build_args: Option<BuildArgsBeta>,
    /// Secrets to add before this deployment.
    /// TODO: Remove this in favour of a separate secrets uploading action.
    pub secrets: Option<HashMap<String, String>>,
    pub build_meta: Option<BuildMetaBeta>,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(tag = "type", content = "content")]
#[typeshare::typeshare]
pub enum BuildArgsBeta {
    Rust(BuildArgsRustBeta),
    #[default]
    Unknown,
}

#[derive(Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct BuildArgsRustBeta {
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

impl Default for BuildArgsRustBeta {
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
#[typeshare::typeshare]
pub struct BuildMetaBeta {
    pub git_commit_id: Option<String>,
    pub git_commit_msg: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

impl std::fmt::Display for BuildMetaBeta {
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
#[typeshare::typeshare]
pub struct DeploymentRequestImageBeta {
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
