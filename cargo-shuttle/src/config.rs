use cargo_metadata::MetadataCommand;
use serde::{Deserialize, Serialize};
use shuttle_common::ApiKey;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use shuttle_common::project::ProjectName;

use crate::args::ProjectArgs;

/// Helper trait for dispatching fs ops for different config files
pub trait ConfigManager: Sized {
    fn directory(&self) -> PathBuf;

    fn file(&self) -> PathBuf;

    fn path(&self) -> PathBuf {
        self.directory().join(self.file())
    }

    fn exists(&self) -> bool {
        self.path().exists()
    }

    fn create<C>(&self) -> Result<()>
    where
        C: Serialize + Default,
    {
        if self.exists() {
            return Ok(());
        }
        let config = C::default();
        self.save(&config)
    }

    fn open<C>(&self) -> Result<C>
    where
        C: for<'de> Deserialize<'de>,
    {
        let path = self.path();
        let config_bytes = File::open(&path).and_then(|mut f| {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Ok(buf)
        })?;
        toml::from_slice(config_bytes.as_slice())
            .with_context(|| anyhow!("Invalid global configuration file: {}", path.display()))
    }

    fn save<C>(&self, config: &C) -> Result<()>
    where
        C: Serialize,
    {
        let path = self.path();
        std::fs::create_dir_all(path.parent().unwrap())?;

        let mut config_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        let config_str = toml::to_string_pretty(config).unwrap();
        config_file.write(config_str.as_bytes()).with_context(|| {
            anyhow!(
                "Could not write the global configuration file: {}",
                path.display()
            )
        })?;
        Ok(())
    }
}

pub struct GlobalConfigManager;

impl ConfigManager for GlobalConfigManager {
    fn directory(&self) -> PathBuf {
        let shuttle_config_dir = dirs::config_dir()
            .ok_or_else(|| {
                anyhow!(
            "Could not find a configuration directory. Your operating system may not be supported."
        )
            })
            .unwrap();
        shuttle_config_dir.join("shuttle")
    }

    fn file(&self) -> PathBuf {
        PathBuf::from("config.toml")
    }
}

/// An impl of [`ConfigManager`] which is localised to a working directory
pub struct LocalConfigManager {
    working_directory: PathBuf,
}

impl LocalConfigManager {
    pub fn new<P: AsRef<Path>>(working_directory: P) -> Self {
        Self {
            working_directory: working_directory.as_ref().to_path_buf(),
        }
    }
}

impl ConfigManager for LocalConfigManager {
    fn directory(&self) -> PathBuf {
        self.working_directory.clone()
    }

    fn file(&self) -> PathBuf {
        PathBuf::from("Shuttle.toml")
    }
}

/// Global client config for things like API keys.
#[derive(Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    pub api_key: Option<ApiKey>,
}

impl GlobalConfig {
    pub fn api_key(&self) -> Option<&ApiKey> {
        self.api_key.as_ref()
    }

    pub fn set_api_key(&mut self, api_key: ApiKey) -> Option<ApiKey> {
        self.api_key.replace(api_key)
    }
}

/// Project-local config for things like customizing project name
#[derive(Deserialize, Serialize, Default)]
pub struct ProjectConfig {
    pub name: Option<ProjectName>,
}

/// A handler for configuration files. The type parameter `M` is the [`ConfigManager`] which handles
/// indirection around file location and serde. The type parameter `C` is the configuration content.
///
/// # Usage
/// ```rust,no_run
///# use cargo_shuttle::config::{Config, GlobalConfigManager, GlobalConfig};
/// let mut config = Config::new(GlobalConfigManager);
/// config.open().unwrap();
/// let content: &GlobalConfig = config.as_ref().unwrap();
/// ```
pub struct Config<M, C> {
    pub manager: M,
    config: Option<C>,
}

impl<M, C> Config<M, C>
where
    M: ConfigManager,
    C: Serialize + for<'de> Deserialize<'de>,
{
    /// Creates a new [`Config`] instance, without opening the underlying file
    pub fn new(manager: M) -> Self {
        Self {
            manager,
            config: None,
        }
    }

    /// Opens the underlying config file, as handled by the [`ConfigManager`]
    pub fn open(&mut self) -> Result<()> {
        let config = self.manager.open()?;
        self.config = Some(config);
        Ok(())
    }

    /// Saves the current state of the config to the file managed by the [`ConfigManager`]
    pub fn save(&self) -> Result<()> {
        self.manager.save(self.config.as_ref().unwrap())
    }

    /// Check if the file managed by the [`ConfigManager`] exists
    pub fn exists(&self) -> bool {
        self.manager.exists()
    }

    /// Replace the current config state with a new value.
    ///
    /// Does not persist the change to disk. Use [`Config::save`] for that.
    pub fn replace(&mut self, config: C) -> Option<C> {
        self.config.replace(config)
    }

    /// Get a mut ref to the underlying config state. Returns `None` if the config has not been
    /// opened.
    pub fn as_mut(&mut self) -> Option<&mut C> {
        self.config.as_mut()
    }

    /// Get a ref to the underlying config state. Returns `None` if the config has not been
    /// opened.
    pub fn as_ref(&self) -> Option<&C> {
        self.config.as_ref()
    }

    /// Ask the [`ConfigManager`] to create a default config file at the location it manages.
    ///
    /// If the file already exists, is a no-op.
    pub fn create(&self) -> Result<()>
    where
        C: Default,
    {
        self.manager.create::<C>()
    }
}

/// A wrapper around our two sources of configuration and overrides:
/// - Global config
/// - Local config
pub struct RequestContext {
    global: Config<GlobalConfigManager, GlobalConfig>,
    project: Option<Config<LocalConfigManager, ProjectConfig>>,
}

fn find_crate_name<P: AsRef<Path>>(working_directory: P) -> Result<ProjectName> {
    let meta = MetadataCommand::new()
        .current_dir(working_directory.as_ref())
        .exec()
        .unwrap();
    let package_name = meta
        .root_package()
        .ok_or_else(|| {
            anyhow!(
                "could not find a root package in `{}`",
                working_directory.as_ref().display()
            )
        })?
        .name
        .clone()
        .parse()?;
    Ok(package_name)
}

impl RequestContext {
    /// Create a [`RequestContext`], only loading in the global configuration details.
    pub fn load_global() -> Result<Self> {
        let mut global = Config::new(GlobalConfigManager);
        if !global.exists() {
            global.create()?;
        }
        global.open()?;
        Ok(Self {
            global,
            project: None,
        })
    }

    /// Load the project configuration at the given `working_directory`
    ///
    /// Ensures that if `--name` is not specified on the command-line, and either the project
    /// file does not exist, or it has not set the `name` key then the `ProjectConfig` instance
    /// has `ProjectConfig.name = Some("crate-name")`.
    pub fn load_local(&mut self, project_args: &ProjectArgs) -> Result<()> {
        let local_manager = LocalConfigManager::new(&project_args.working_directory);
        let mut project = Config::new(local_manager);
        if !project.exists() {
            project.replace(ProjectConfig::default());
        } else {
            project.open()?;
        };

        let config = project.as_mut().unwrap();
        match (&project_args.name, &config.name) {
            // Command-line name parameter trumps everything
            (Some(name_from_args), _) => config.name = Some(name_from_args.clone()),
            // If key exists in config then keep it as it is
            (None, Some(_)) => {}
            // If name key is not in project config, then we infer from crate name
            (None, None) => config.name = Some(find_crate_name(&project_args.working_directory)?),
        };

        self.project = Some(project);

        Ok(())
    }

    /// Get the API key from the global configuration. Returns an error if API key not set in there.
    pub fn api_key(&self) -> Result<&ApiKey> {
        self.global.as_ref().unwrap().api_key().ok_or_else(|| {
            anyhow!(
                "Configuration file: `{}`",
                self.global.manager.path().display()
            )
            .context(anyhow!("No valid API key found, try logging in first."))
        })
    }

    /// Get the current context working directory
    ///
    /// # Panics
    /// Panics if project configuration has not been loaded.
    pub fn working_directory(&self) -> &Path {
        self.project
            .as_ref()
            .unwrap()
            .manager
            .working_directory
            .as_path()
    }

    /// Set the API key to the global configuration. Will persist the file.
    pub fn set_api_key(&mut self, api_key: ApiKey) -> Result<Option<ApiKey>> {
        let res = self.global.as_mut().unwrap().set_api_key(api_key);
        self.global.save()?;
        Ok(res)
    }

    /// Get the current project name.
    ///
    /// # Panics
    /// Panics if the project configuration has not been loaded.
    pub fn project_name(&self) -> &ProjectName {
        self.project
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .name
            .as_ref()
            .unwrap()
    }
}
