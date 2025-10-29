pub mod args;
pub mod commands;
pub mod config;

use std::collections::HashMap;

use anyhow::{bail, Result};
use hyper::HeaderMap;
use shuttle_api_client::{impulse::ImpulseClient, ShuttleApiClient};
use shuttle_common::constants::headers::X_CARGO_SHUTTLE_VERSION;
use tracing_subscriber::{reload::Handle, EnvFilter, Registry};

use crate::{
    impulse::{
        args::{GenerateCommand, ImpulseCommand, ImpulseGlobalArgs},
        config::ConfigLayers,
    },
    reload_env_filter,
};

pub enum ImpulseCommandOutput {
    BuiltImage(String),
    None,
}

pub struct Impulse {
    config: ConfigLayers,
    client: Option<ImpulseClient>,
    global_args: ImpulseGlobalArgs,
    // /// Alter behaviour based on which CLI is used
    // bin: Binary,
}

impl Impulse {
    pub fn new(
        global_args: ImpulseGlobalArgs,
        // bin: Binary,
        // env_override: Option<String>,
        env_filter_handle: Option<Handle<EnvFilter, Registry>>,
    ) -> Result<Self> {
        let mut config = ConfigLayers::new(global_args.clone());

        // Load config files and refresh the env filter based on the potentially new debug value
        // TODO?: move this out? resolve config earlier?
        if let Some(ref handle) = env_filter_handle {
            reload_env_filter(handle, config.get_config()?.debug);
        }

        Ok(Self {
            config,
            client: None,
            global_args,
            // bin,
        })
    }

    pub async fn run(mut self, command: ImpulseCommand) -> Result<ImpulseCommandOutput> {
        use ImpulseCommand::*;

        // For all commands that call an API, initiate the client if it has not yet been done
        if matches!(command, Deploy(_) | Login(_)) {
            if self.client.is_none() {
                self.client = Some(self.make_api_client()?);
            }
            if self
                .client
                .as_ref()
                .is_some_and(|c| c.inner.api_key.is_none())
            {
                bail!("No API key found. Log in with `impulse login` or set the `IMPULSE_API_KEY` env var.")
            }
        }

        match command {
            Init(init_args) => self.init(init_args).await,
            Build(build_args) => self.build(build_args).await,
            Run(run_args) => self.local_run(run_args).await,
            Deploy(deploy_args) => self.deploy(deploy_args).await,
            Login(login_args) => self.login(login_args).await,
            Logout(logout_args) => self.logout(logout_args).await,
            Generate(cmd) => match cmd {
                GenerateCommand::Shell { shell, output_file } => {
                    self.generate_completions(shell, output_file).await
                }
                GenerateCommand::Manpage { output_file } => {
                    self.generate_manpage(output_file).await
                }
                GenerateCommand::Agents => self.generate_agents().await,
            },
            Upgrade { preview } => self.self_upgrade(preview).await,
        }
    }

    /// Create a new API client based on the current values in config
    pub fn make_api_client(&mut self) -> Result<ImpulseClient> {
        let config = self.config.get_config()?;

        Ok(ImpulseClient {
            inner: ShuttleApiClient::new(
                config.api_url,
                config.api_key,
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
