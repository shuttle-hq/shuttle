use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// Helper trait for dispatching fs ops for different config files
pub trait ConfigManager: Sized {
    fn directory(&self) -> PathBuf;

    fn filename(&self) -> PathBuf;

    fn path(&self) -> PathBuf {
        self.directory().join(self.filename())
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
        let config_string = File::open(&path)
            .and_then(|mut f| {
                let mut buf = String::new();
                f.read_to_string(&mut buf)?;
                Ok(buf)
            })
            .with_context(|| anyhow!("Unable to read configuration file: {}", path.display()))?;
        toml::from_str(config_string.as_str())
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

pub struct GlobalConfigManager {
    env_override: Option<String>,
}

impl GlobalConfigManager {
    pub fn new(env_override: Option<String>) -> Result<Self> {
        if let Some(ref s) = env_override {
            if s.chars().any(|c| !c.is_ascii_alphanumeric()) {
                return Err(anyhow!("Invalid Shuttle API Environment name"));
            }
        }

        Ok(Self { env_override })
    }
}

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

    fn filename(&self) -> PathBuf {
        match self.env_override.as_ref() {
            Some(env) => PathBuf::from(format!("config.{env}.toml")),
            None => PathBuf::from("config.toml"),
        }
    }
}

/// Global client config for things like API keys.
#[derive(Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    pub api_key: Option<String>,
    // mostly unused but can still be used if needed
    pub api_url: Option<String>,
}

/// A handler for configuration files. The type parameter `M` is the [`ConfigManager`] which handles
/// indirection around file location and serde. The type parameter `C` is the configuration content.
///
/// # Usage
/// ```rust,no_run
/// # use cargo_shuttle::config::{Config, GlobalConfig, GlobalConfigManager};
/// #
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
