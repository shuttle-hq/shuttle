use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use shuttle_common::config::{Config, ConfigManager, GlobalConfig, GlobalConfigManager};
use shuttle_common::constants::SHUTTLE_API_URL;
use tracing::trace;

use crate::args::ProjectArgs;
use crate::init::create_or_update_ignore_file;

/// An impl of [`ConfigManager`] which is localised to a working directory
pub struct LocalConfigManager {
    directory: PathBuf,
    file_name: String,
}

impl LocalConfigManager {
    pub fn new<P: AsRef<Path>>(directory: P, file_name: String) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
            file_name,
        }
    }
}

impl ConfigManager for LocalConfigManager {
    fn directory(&self) -> PathBuf {
        self.directory.clone()
    }

    fn filename(&self) -> PathBuf {
        PathBuf::from(&self.file_name)
    }
}

/// Shuttle.toml schema (User-facing project-local config)
#[derive(Deserialize, Serialize, Default)]
pub struct ProjectConfig {
    // unused on new platform, but still used for project names in local runs
    // TODO: remove and use the crate/workspace name instead
    pub name: Option<String>,
    /// Deprecated, now [`ProjectDeployConfig::include`]
    pub assets: Option<Vec<String>>,
    pub deploy: Option<ProjectDeployConfig>,
    pub build: Option<ProjectBuildConfig>,
}
/// Deployment command config
#[derive(Deserialize, Serialize, Default)]
pub struct ProjectDeployConfig {
    /// Successor to `assets`.
    /// Patterns of ignored files that should be included in deployments.
    pub include: Option<Vec<String>>,
    /// Set to true to deny deployments with uncommited changes. (use `--allow-dirty` to override)
    pub deny_dirty: Option<bool>,
}
/// Builder config
#[derive(Deserialize, Serialize, Default)]
pub struct ProjectBuildConfig {
    /// Successor to `build_assets`.
    /// Patterns of files that should be copied from the build to the runtime container.
    pub assets: Option<Vec<String>>,
}

/// .shuttle/config.toml schema (internal project-local config)
#[derive(Deserialize, Serialize, Default)]
pub struct InternalProjectConfig {
    // should be in internal local config
    pub id: Option<String>,
}

/// A wrapper around our two sources of configuration and overrides:
/// - Global config
/// - Local config
pub struct RequestContext {
    global: Config<GlobalConfigManager, GlobalConfig>,
    project: Option<Config<LocalConfigManager, ProjectConfig>>,
    project_internal: Option<Config<LocalConfigManager, InternalProjectConfig>>,
    api_url: Option<String>,
}

impl RequestContext {
    /// Create a [`RequestContext`], only loading in the global configuration details.
    pub fn load_global(env_override: Option<String>) -> Result<Self> {
        let mut global = Config::new(GlobalConfigManager::new(env_override)?);
        if !global.exists() {
            global.create()?;
        }
        global
            .open()
            .context("Unable to load global configuration")?;
        Ok(Self {
            global,
            project: None,
            project_internal: None,
            api_url: None,
        })
    }

    pub fn load_local_internal_config(&mut self, project_args: &ProjectArgs) -> Result<()> {
        let workspace_path = project_args
            .workspace_path()
            .unwrap_or(project_args.working_directory.clone());

        trace!(
            "looking for .shuttle/config.toml in {}",
            workspace_path.display()
        );
        let local_manager =
            LocalConfigManager::new(workspace_path, ".shuttle/config.toml".to_string());
        let mut project_internal = Config::new(local_manager);
        if !project_internal.exists() {
            trace!("no local .shuttle/config.toml found");
            project_internal.replace(InternalProjectConfig::default());
        } else {
            trace!("found a local .shuttle/config.toml");
            project_internal.open()?;
        }

        let config = project_internal.as_mut().unwrap();

        // Project id is preferred in this order:
        // 1. Id given on command line
        // 2. Id from .shuttle/config.toml file
        match (&project_args.id, &config.id) {
            // Command-line id parameter trumps everything
            (Some(id_from_args), _) => {
                trace!("using command-line project id");

                // Validate format of explicitly given project id and change the ULID to uppercase if it is lowercase
                let id_to_use = if let Some(proj_id_uppercase) =
                    id_from_args.strip_prefix("proj_").and_then(|suffix| {
                        // Soft (dumb) validation of ULID format (ULIDs are 26 chars)
                        (suffix.len() == 26)
                            .then_some(format!("proj_{}", suffix.to_ascii_uppercase()))
                    }) {
                    if *id_from_args != proj_id_uppercase {
                        eprintln!("INFO: Converted project id to '{}'", proj_id_uppercase);

                        proj_id_uppercase
                    } else {
                        id_from_args.clone()
                    }
                } else {
                    // TODO: eprintln a warning?
                    tracing::warn!("project id with bad format detected: '{id_from_args}'");

                    id_from_args.clone()
                };

                config.id = Some(id_to_use);
            }
            // If key exists in config then keep it as it is
            (None, Some(_)) => {
                trace!("using .shuttle/config.toml project id");
            }
            (None, None) => {
                trace!("no project id in args or config found");
            }
        };

        self.project_internal = Some(project_internal);

        Ok(())
    }

    pub fn set_project_id(&mut self, id: String) {
        *self.project_internal.as_mut().unwrap().as_mut().unwrap() =
            InternalProjectConfig { id: Some(id) };
    }

    pub fn remove_project_id(&mut self) {
        *self.project_internal.as_mut().unwrap().as_mut().unwrap() =
            InternalProjectConfig { id: None };
    }

    pub fn save_local_internal(&mut self) -> Result<()> {
        self.project_internal.as_ref().unwrap().save()?;

        // write updated gitignore file to root of workspace
        // TODO: assumes git is used
        create_or_update_ignore_file(
            &self
                .project
                .as_ref()
                .unwrap()
                .manager
                .directory
                .join(".gitignore"),
        )
        .context("Failed to create .gitignore file")?;

        Ok(())
    }

    /// Load the Shuttle.toml project configuration at the given `working_directory`
    pub fn load_local_config(&mut self, project_args: &ProjectArgs) -> Result<()> {
        self.project = Some(Self::get_local_config(project_args)?);

        Ok(())
    }

    fn get_local_config(
        project_args: &ProjectArgs,
    ) -> Result<Config<LocalConfigManager, ProjectConfig>> {
        let workspace_path = project_args
            .workspace_path()
            .unwrap_or(project_args.working_directory.clone());

        trace!("looking for Shuttle.toml in {}", workspace_path.display());

        // check that the uppercase file does not exist so a false warning is not printed on case-insensitive file systems
        if !workspace_path.join("Shuttle.toml").exists()
            && workspace_path.join("shuttle.toml").exists()
        {
            eprintln!("WARN: Lowercase 'shuttle.toml' detected, please use 'Shuttle.toml'")
        }

        let local_manager = LocalConfigManager::new(workspace_path, "Shuttle.toml".to_string());
        let mut project = Config::new(local_manager);
        if !project.exists() {
            trace!("no local Shuttle.toml found");
            project.replace(ProjectConfig::default());
        } else {
            trace!("found a local Shuttle.toml");
            project.open()?;
        }

        let config = project.as_mut().unwrap();

        // Project names are preferred in this order:
        // 1. Name given on command line
        // 2. Name from Shuttle.toml file
        // 3. Name from Cargo.toml package if it's a crate
        // 4. Name from the workspace directory if it's a workspace
        match (&project_args.name, &config.name) {
            // Command-line name parameter trumps everything
            (Some(name_from_args), _) => {
                trace!("using command-line project name");
                config.name = Some(name_from_args.clone());
            }
            // If key exists in config then keep it as it is
            (None, Some(_)) => {
                trace!("using Shuttle.toml project name");
            }
            // If name key is not in project config, then we infer from crate name
            (None, None) => {
                trace!("using crate name as project name");
                config.name = Some(project_args.project_name()?);
            }
        };
        // now, `config.name` is always Some

        Ok(project)
    }

    pub fn set_api_url(&mut self, api_url: Option<String>) {
        self.api_url = api_url;
    }

    pub fn api_url(&self) -> String {
        if let Some(api_url) = self.api_url.clone() {
            api_url
        } else if let Some(api_url) = self.global.as_ref().unwrap().api_url.clone() {
            api_url
        } else {
            SHUTTLE_API_URL.to_string()
        }
    }

    /// Get the API key from the `SHUTTLE_API_KEY` env variable, or
    /// otherwise from the global configuration. Returns an error if
    /// an API key is not set.
    pub fn api_key(&self) -> Result<String> {
        match std::env::var("SHUTTLE_API_KEY") {
            Ok(key) => Ok(key),
            Err(_) => match self.global.as_ref().unwrap().api_key.clone() {
                Some(key) => Ok(key),
                None => Err(anyhow!(
                    "Configuration file: `{}`",
                    self.global.manager.path().display()
                )
                .context("No valid API key found, try logging in with `shuttle login`")),
            },
        }
    }

    /// Get the cargo workspace root directory
    ///
    /// # Panics
    /// Panics if project configuration has not been loaded.
    pub fn project_directory(&self) -> &Path {
        self.project.as_ref().unwrap().manager.directory.as_path()
    }

    /// Set the API key to the global configuration. Will persist the file.
    pub fn set_api_key(&mut self, api_key: String) -> Result<()> {
        self.global.as_mut().unwrap().api_key = Some(api_key);
        self.global.save()
    }

    pub fn clear_api_key(&mut self) -> Result<()> {
        self.global.as_mut().unwrap().api_key = None;
        self.global.save()
    }

    /// Get the current project name.
    ///
    /// # Panics
    /// Panics if the project configuration has not been loaded.
    pub fn project_name(&self) -> &str {
        self.project
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .name
            .as_ref()
            .unwrap()
            .as_str()
    }

    /// # Panics
    /// Panics if the project configuration has not been loaded.
    pub fn include(&self) -> Option<&Vec<String>> {
        self.project
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .deploy
            .as_ref()
            .and_then(|d| d.include.as_ref())
            .or(self
                .project
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .assets
                .as_ref())
    }

    /// # Panics
    /// Panics if the project configuration has not been loaded.
    pub fn deny_dirty(&self) -> Option<bool> {
        self.project
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .deploy
            .as_ref()
            .and_then(|d| d.deny_dirty)
    }

    /// Check if the current project id has been loaded.
    pub fn project_id_found(&self) -> bool {
        self.project_internal
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .id
            .is_some()
    }

    /// Get the current project id.
    ///
    /// # Panics
    /// Panics if the internal project configuration has not been loaded.
    pub fn project_id(&self) -> &str {
        self.project_internal
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .id
            .as_ref()
            .unwrap()
            .as_str()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{args::ProjectArgs, config::RequestContext};

    use super::{Config, LocalConfigManager, ProjectConfig};

    fn path_from_workspace_root(path: &str) -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path)
    }

    fn unwrap_project_name(config: &Config<LocalConfigManager, ProjectConfig>) -> String {
        config.as_ref().unwrap().name.as_ref().unwrap().to_string()
    }

    #[test]
    fn get_local_config_finds_name_in_cargo_toml() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/"),
            name: None,
            id: None,
        };

        let local_config = RequestContext::get_local_config(&project_args).unwrap();

        assert_eq!(unwrap_project_name(&local_config), "hello-world");
    }

    #[test]
    fn get_local_config_finds_name_from_workspace_dir() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/rocket/workspace/hello-world/"),
            name: None,
            id: None,
        };

        let local_config = RequestContext::get_local_config(&project_args).unwrap();

        assert_eq!(unwrap_project_name(&local_config), "workspace");
    }

    #[test]
    fn setting_name_overrides_name_in_config() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/"),
            name: Some("my-fancy-project-name".to_owned()),
            id: None,
        };

        let local_config = RequestContext::get_local_config(&project_args).unwrap();

        assert_eq!(unwrap_project_name(&local_config), "my-fancy-project-name");
    }
}
