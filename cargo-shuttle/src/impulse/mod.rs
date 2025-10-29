pub mod args;
pub mod commands;
pub mod config;

use anyhow::Result;
use shuttle_api_client::impulse::ImpulseClient;
use tracing_subscriber::{reload::Handle, EnvFilter, Registry};

use crate::{
    impulse::{
        args::{GenerateCommand, ImpulseCommand, ImpulseGlobalArgs},
        config::ConfigHandler,
    },
    reload_env_filter,
};

pub enum ImpulseCommandOutput {
    BuiltImage(String),
    None,
}

pub struct Impulse {
    config: ConfigHandler,
    client: ImpulseClient,
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
        let config = ConfigHandler::new(global_args.clone())?;

        // Load config files and refresh the env filter based on the potentially new debug value
        if let Some(ref handle) = env_filter_handle {
            reload_env_filter(handle, config.config().debug);
        }

        // Initiate API client
        let client = config.make_api_client()?;

        Ok(Self {
            config,
            client,
            global_args,
            // bin,
        })
    }

    pub async fn run(mut self, command: ImpulseCommand) -> Result<ImpulseCommandOutput> {
        use ImpulseCommand::*;

        // TODO?: warning or error when running commands that need the api key:
        // if matches!(command, ...) && client.inner.api_key.is_none()
        // {
        //     bail!("No API key found. Log in with `impulse login` or set the `IMPULSE_API_KEY` env var.")
        // }

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

    pub fn refresh_api_client(&mut self) -> Result<()> {
        self.client = self.config.make_api_client()?;
        Ok(())
    }
}
