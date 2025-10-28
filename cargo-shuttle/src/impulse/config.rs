use serde::{Deserialize, Serialize};
use shuttle_common::{
    config::{ConfigManager, GlobalConfigManager},
    constants::IMPULSE_API_URL,
};

use crate::{args::OutputMode, config::LocalConfigManager, impulse::args::ImpulseGlobalArgs};

#[derive(Default, Debug, Serialize, Deserialize)]
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
}

impl ImpulseConfig {
    /// Create a new [`ImpulseConfig`] with the values in `other` overriding the values in `self`
    pub fn merge_with(self, other: ImpulseConfig) -> Self {
        Self {
            api_url: other.api_url.or(self.api_url),
            api_key: other.api_key.or(self.api_key),
            debug: other.debug.or(self.debug),
            output_mode: other.output_mode.or(self.output_mode),
        }
    }
}

pub struct ConfigLayers {
    pub global: GlobalConfigManager,
    pub local: LocalConfigManager,
    pub local_internal: LocalConfigManager,
}

impl ConfigLayers {
    pub fn new(global_args: &ImpulseGlobalArgs) -> Self {
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
        }
    }

    /// Read and resolve config values in the order:
    /// - Global config (~/.config/impulse/config.toml)
    /// - Local config (Impulse.toml)
    /// - Local "internal" config (.impulse/config.toml)
    /// - Env vars
    /// - CLI args
    // TODO?: other return type with guaranteed Some() values replaced with non-Options?
    pub fn resolve_config(&self, global_args: ImpulseGlobalArgs) -> ImpulseConfig {
        let mut config = ImpulseConfig::default_values();

        if self.global.exists() {
            if let Ok(globals) = self.global.open::<ImpulseConfig>() {
                config = config.merge_with(globals);
            }
        }
        if self.local.exists() {
            if let Ok(locals) = self.local.open::<ImpulseConfig>() {
                config = config.merge_with(locals);
            }
        }
        if self.local_internal.exists() {
            if let Ok(locals_int) = self.local_internal.open::<ImpulseConfig>() {
                config = config.merge_with(locals_int);
            }
        }

        config = config.merge_with(global_args.into_config());

        config
    }
}
