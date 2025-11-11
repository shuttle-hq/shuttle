pub mod args;
pub mod commands;
pub mod config;
pub mod templates;
pub mod ui;

use anyhow::Result;
use cargo_shuttle::reload_env_filter;
use impulse_common::types::{AggregateProjectCondition, ProjectSpec};
use serde::de::Error;
use shuttle_api_client::neptune::NeptuneClient;
use tracing_subscriber::{reload::Handle, EnvFilter, Registry};

use crate::{
    args::{GenerateCommand, NeptuneCommand, NeptuneGlobalArgs},
    config::ConfigHandler,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub enum NeptuneCommandOutput {
    BuiltImage(String),
    ProjectStatus(Box<AggregateProjectCondition>),
    None,
}

pub struct Neptune {
    config: ConfigHandler,
    client: NeptuneClient,
    global_args: NeptuneGlobalArgs,
    // /// Alter behaviour based on which CLI is used
    // bin: Binary,
}

impl Neptune {
    pub fn new(
        global_args: NeptuneGlobalArgs,
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

    pub async fn run(mut self, command: NeptuneCommand) -> Result<NeptuneCommandOutput> {
        use NeptuneCommand::*;

        // TODO?: warning or error when running commands that need the api key:
        // if matches!(command, ...) && client.inner.api_key.is_none()
        // {
        //     bail!("No API key found. Log in with `neptune login` or set the `NEPTUNE_API_KEY` env var.")
        // }

        match command {
            Init(init_args) => self.init(init_args).await,
            // Build(build_args) => self.build(build_args).await,
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
                GenerateCommand::Agents => {
                    self.generate_agents(&self.global_args.working_directory)
                        .await
                }
                GenerateCommand::Spec => {
                    self.generate_spec(&self.global_args.working_directory)
                        .await
                }
            },
            Upgrade { preview } => self.self_upgrade(preview).await,
            Status => self.status().await,
        }
    }

    pub fn refresh_api_client(&mut self) -> Result<()> {
        self.client = self.config.make_api_client()?;
        Ok(())
    }

    pub(crate) async fn fetch_local_state(
        &self,
    ) -> std::result::Result<ProjectSpec, std::io::Error> {
        if tokio::fs::try_exists("shuttle.json").await? {
            let bytes = tokio::fs::read("shuttle.json").await?;
            let root: serde_json::Value = serde_json::from_slice(&bytes)?;
            if let Some(spec) = root.get("spec") {
                Ok(serde_json::from_value(spec.clone())?)
            } else {
                Err(std::io::Error::other(serde_json::Error::missing_field(
                    "spec",
                )))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Missing 'shuttle.json'",
            ))
        }
    }
}
