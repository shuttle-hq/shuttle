use anyhow::Context;
use serde::{Deserialize, Serialize};
use shuttle_common::{
    config::{ConfigManager, GlobalConfigManager},
    constants::IMPULSE_API_URL,
};

use crate::{args::OutputMode, config::LocalConfigManager, impulse::args::ImpulseGlobalArgs};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ImpulseConfig {
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub debug: Option<bool>,
    pub output_mode: Option<OutputMode>,
}

impl ImpulseConfig {
    /// `Default::default()` is used for all-None config. This is used for default values when none are set.
    pub fn default_values() -> Self {
        Self {
            api_url: Some(IMPULSE_API_URL.to_owned()),
            api_key: None,
            debug: Some(false),
            output_mode: Some(OutputMode::Normal),
        }
    }

    /// Create a new [`ImpulseConfig`] with the values in `other` overriding the values in `self`
    pub fn merge_with(self, other: ImpulseConfig) -> Self {
        Self {
            api_url: other.api_url.or(self.api_url),
            api_key: other.api_key.or(self.api_key),
            debug: other.debug.or(self.debug),
            output_mode: other.output_mode.or(self.output_mode),
        }
    }

    /// Assume all non-optional fields have been set and convert to more convenient type
    pub fn into_resolved(self) -> anyhow::Result<ResolvedImpulseConfig> {
        Ok(ResolvedImpulseConfig {
            api_url: self
                .api_url
                .context("missing api_url when resolving config")?,
            api_key: self.api_key,
            debug: self.debug.context("missing debug when resolving config")?,
            output_mode: self
                .output_mode
                .context("missing output_mode when resolving config")?,
        })
    }
}

/// Same as `ImpulseConfig`, but all non-optional fields are not options
#[derive(Debug, Clone)]
pub struct ResolvedImpulseConfig {
    pub api_url: String,
    pub api_key: Option<String>,
    pub debug: bool,
    pub output_mode: OutputMode,
}

pub struct ConfigLayers {
    global: GlobalConfigManager,
    local: LocalConfigManager,
    local_internal: LocalConfigManager,
    args_config: ImpulseConfig,

    resolved: Option<ResolvedImpulseConfig>,
}

impl ConfigLayers {
    pub fn new(global_args: ImpulseGlobalArgs) -> Self {
        Self {
            global: GlobalConfigManager::new("impulse".to_owned(), None)
                .expect("No environments in impulse yet"),
            local_internal: LocalConfigManager::new(
                global_args.working_directory.join(".impulse"),
                "config.toml".to_owned(),
            ),
            local: LocalConfigManager::new(
                global_args.working_directory.clone(),
                "Impulse.toml".to_owned(),
            ),
            args_config: global_args.into_config(),
            resolved: None,
        }
    }

    pub fn get_config(&mut self) -> anyhow::Result<ResolvedImpulseConfig> {
        if self.resolved.is_none() {
            self.resolve_config()?;
        }
        Ok(self
            .resolved
            .clone()
            .context("expected config to be resolved")?)
    }

    /// Read and resolve config values in the order:
    /// - Global config (~/.config/impulse/config.toml)
    /// - Local config (Impulse.toml)
    /// - Local "internal" config (.impulse/config.toml)
    /// - Env vars
    /// - CLI args
    fn resolve_config(&mut self) -> anyhow::Result<()> {
        let mut config = ImpulseConfig::default_values();

        if self.global.exists() {
            tracing::debug!(file = %self.global.path().display(), "Found config file");
            if let Ok(globals) = self.global.open::<ImpulseConfig>() {
                config = config.merge_with(globals);
            }
        }
        if self.local.exists() {
            tracing::debug!(file = %self.local.path().display(), "Found config file");
            if let Ok(locals) = self.local.open::<ImpulseConfig>() {
                config = config.merge_with(locals);
            }
        }
        if self.local_internal.exists() {
            tracing::debug!(file = %self.local_internal.path().display(), "Found config file");
            if let Ok(locals_int) = self.local_internal.open::<ImpulseConfig>() {
                config = config.merge_with(locals_int);
            }
        }

        config = config.merge_with(self.args_config.clone());
        let resolved = config.into_resolved()?;

        tracing::debug!(config = ?resolved, "Resolved config");

        self.resolved = Some(resolved);

        Ok(())
    }

    pub fn modify_global<F>(&mut self, mut f: F) -> anyhow::Result<()>
    where
        F: FnMut(&mut ImpulseConfig) -> (),
    {
        let mut global = self.global.open::<ImpulseConfig>().unwrap_or_default();
        f(&mut global);
        self.global.save(&global)?;
        Ok(())
    }
}
