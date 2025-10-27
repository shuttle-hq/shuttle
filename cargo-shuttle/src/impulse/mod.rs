pub mod args;
pub mod commands;

use anyhow::Result;
use shuttle_api_client::impulse::ImpulseClient;

use crate::impulse::args::{GenerateCommand, ImpulseCommand, ImpulseGlobalArgs};

pub enum ImpulseCommandOutput {
    BuiltImage(String),
    None,
}

pub struct Impulse {
    // ctx: RequestContext,
    _client: Option<ImpulseClient>,
    global_args: ImpulseGlobalArgs,
    // /// Alter behaviour based on which CLI is used
    // bin: Binary,
}

impl Impulse {
    pub fn new(
        global_args: ImpulseGlobalArgs, /* bin: Binary */ /* env_override: Option<String> */
    ) -> Result<Self> {
        // let ctx = RequestContext::load_global(env_override.inspect(|e| {
        //     eprintln!(
        //         "{}",
        //         format!("INFO: Using non-default global config file: {e}").yellow(),
        //     )
        // }))?;
        Ok(Self {
            // ctx,
            _client: None,
            global_args,
            // bin,
        })
    }

    pub async fn run(self, command: ImpulseCommand) -> Result<ImpulseCommandOutput> {
        use ImpulseCommand::*;
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
}
