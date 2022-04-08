mod args;
mod client;
mod config;

use crate::args::{Args, AuthArgs, DeployArgs};
use crate::config::RequestContext;
use anyhow::{Context, Result};
use args::LoginArgs;
use cargo::core::resolver::CliFeatures;
use cargo::core::Workspace;
use cargo::ops::{PackageOpts, Packages};

use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::{env, io};
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    Shuttle::new().run(Args::from_args()).await
}

pub struct Shuttle {
    ctx: RequestContext,
}

impl Default for Shuttle {
    fn default() -> Self {
        Self::new()
    }
}

impl Shuttle {
    pub fn new() -> Self {
        let ctx = RequestContext::load_global().unwrap();
        Self { ctx }
    }

    pub async fn run(mut self, args: Args) -> Result<()> {
        if matches!(args, Args::Deploy(..) | Args::Delete | Args::Status) {
            self.load_project()?;
        }

        match args {
            Args::Deploy(deploy_args) => self.deploy(deploy_args).await,
            Args::Status => self.status().await,
            Args::Delete => self.delete().await,
            Args::Auth(auth_args) => self.auth(auth_args).await,
            Args::Login(login_args) => self.login(login_args).await,
        }
    }

    pub fn load_project(&mut self) -> Result<()> {
        let working_directory = env::current_dir()?;
        self.ctx.load_local(working_directory)
    }

    async fn login(&mut self, login_args: LoginArgs) -> Result<()> {
        let api_key_str = login_args.api_key.unwrap_or_else(|| {
            let url = "https://shuttle.rs/login";

            let _ = webbrowser::open(url);

            println!("If your browser did not automatically open, go to {url}");
            print!("Enter Api Key: ");

            io::stdout().flush().unwrap();

            let mut input = String::new();

            io::stdin().read_line(&mut input).unwrap();

            input
        });

        let api_key = api_key_str.trim().parse()?;

        self.ctx.set_api_key(api_key)?;

        Ok(())
    }

    async fn auth(&mut self, auth_args: AuthArgs) -> Result<()> {
        let api_key = client::auth(self.ctx.api_url(), auth_args.username)
            .await
            .context("failed to retrieve api key")?;
        self.ctx.set_api_key(api_key)?;
        Ok(())
    }

    async fn delete(&self) -> Result<()> {
        client::delete(
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to delete deployment")
    }

    async fn status(&self) -> Result<()> {
        client::status(
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to get status of deployment")
    }

    async fn deploy(&self, args: DeployArgs) -> Result<()> {
        let package_file = self
            .run_cargo_package(args.allow_dirty)
            .context("failed to package cargo project")?;
        client::deploy(
            package_file,
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to deploy cargo project")
    }

    // Packages the cargo project and returns a File to that file
    fn run_cargo_package(&self, allow_dirty: bool) -> Result<File> {
        let config = cargo::util::config::Config::default()?;

        let working_directory = self.ctx.working_directory();
        let path = working_directory.join("Cargo.toml");

        let ws = Workspace::new(&path, &config)?;
        let opts = PackageOpts {
            config: &config,
            list: false,
            check_metadata: true,
            allow_dirty,
            verify: false,
            jobs: None,
            to_package: Packages::Default,
            targets: vec![],
            cli_features: CliFeatures {
                features: Rc::new(Default::default()),
                all_features: false,
                uses_default_features: true,
            },
        };

        let locks = cargo::ops::package(&ws, &opts)?.expect("unwrap ok here");
        let owned = locks.get(0).unwrap().file().try_clone()?;
        Ok(owned)
    }
}
