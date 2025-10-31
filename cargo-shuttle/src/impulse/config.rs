use std::collections::HashMap;

use anyhow::{Context, Result};
use hyper::HeaderMap;
use serde::{Deserialize, Serialize};
use shuttle_api_client::{impulse::ImpulseClient, ShuttleApiClient};
use shuttle_common::{
    config::{ConfigManager, GlobalConfigManager},
    constants::{headers::X_CARGO_SHUTTLE_VERSION, IMPULSE_API_URL},
};

use crate::{args::OutputMode, config::LocalConfigManager, impulse::args::ImpulseGlobalArgs};

/// Schema for each config file. Everything is optional.
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

pub struct ConfigHandler {
    global: GlobalConfigManager,
    _local: LocalConfigManager,
    _local_internal: LocalConfigManager,

    resolved: ResolvedImpulseConfig,
}

impl ConfigHandler {
    pub fn new(global_args: ImpulseGlobalArgs) -> Result<Self> {
        let global = GlobalConfigManager::new("impulse".to_owned(), None)
            .expect("No environments in impulse yet");
        let local_internal = LocalConfigManager::new(
            global_args.working_directory.join(".impulse"),
            "config.toml".to_owned(),
        );
        let local = LocalConfigManager::new(
            global_args.working_directory.clone(),
            "Impulse.toml".to_owned(),
        );

        let resolved =
            Self::resolve_config(&global, &local, &local_internal, global_args.into_config())?;

        Ok(Self {
            global,
            _local_internal: local_internal,
            _local: local,
            resolved,
        })
    }

    /// Read and resolve config values in the order:
    /// - Global config (~/.config/impulse/config.toml)
    /// - Local config (Impulse.toml)
    /// - Local "internal" config (.impulse/config.toml)
    /// - Env vars
    /// - CLI args
    fn resolve_config(
        global: &GlobalConfigManager,
        local: &LocalConfigManager,
        local_internal: &LocalConfigManager,
        args_config: ImpulseConfig,
    ) -> Result<ResolvedImpulseConfig> {
        let mut config = ImpulseConfig::default_values();

        if global.exists() {
            tracing::debug!(file = %global.path().display(), "Reading config file");
            if let Ok(globals) = global.open::<ImpulseConfig>() {
                config = config.merge_with(globals);
            }
        }
        if local.exists() {
            tracing::debug!(file = %local.path().display(), "Reading config file");
            if let Ok(locals) = local.open::<ImpulseConfig>() {
                config = config.merge_with(locals);
            }
        }
        if local_internal.exists() {
            tracing::debug!(file = %local_internal.path().display(), "Reading config file");
            if let Ok(locals_int) = local_internal.open::<ImpulseConfig>() {
                config = config.merge_with(locals_int);
            }
        }

        config = config.merge_with(args_config);
        let resolved = config.into_resolved()?;

        tracing::debug!(config = ?resolved, "resolved config");

        Ok(resolved)
    }

    pub fn config(&self) -> &ResolvedImpulseConfig {
        &self.resolved
    }

    pub fn modify_global<F>(&mut self, mut f: F) -> Result<()>
    where
        F: FnMut(&mut ImpulseConfig) -> (),
    {
        let mut global = self.global.open::<ImpulseConfig>().unwrap_or_default();
        f(&mut global);
        tracing::debug!("saving global config");
        self.global.save(&global)?;
        Ok(())
    }

    /// Create a new API client based on this config's values
    pub fn make_api_client(&self) -> Result<ImpulseClient> {
        let config = self.config();
        Ok(ImpulseClient {
            inner: ShuttleApiClient::new(
                config.api_url.clone(),
                config.api_key.clone(),
                Some(
                    HeaderMap::try_from(&HashMap::from([(
                        X_CARGO_SHUTTLE_VERSION.clone(),
                        crate::VERSION.to_owned(),
                    )]))
                    .unwrap(),
                ),
                None,
            ),
        })
    }
}
