mod args;
mod client;
pub mod config;
mod factory;
mod print;

use std::fs::File;
use std::io::Write;
use std::io::{self, stdout};
use std::net::{Ipv4Addr, SocketAddr};
use std::process::Command as PCmd;
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
pub use args::{Args, Command, ProjectArgs, RunArgs};
use args::{AuthArgs, DeployArgs, LoginArgs};
use cargo::core::compiler::CompileMode;
use cargo::core::resolver::CliFeatures;
use cargo::core::Workspace;
use cargo::ops::{CompileOptions, PackageOpts, Packages, TestOptions};
use colored::Colorize;
use config::RequestContext;
use factory::LocalFactory;
use futures::future::TryFutureExt;
use semver::Version;
use shuttle_service::loader::{build_crate, Loader};
use tokio::sync::mpsc;
use uuid::Uuid;

#[macro_use]
extern crate log;

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
        trace!("running local client");
        if matches!(
            args.cmd,
            Command::Deploy(..)
                | Command::Delete
                | Command::Status
                | Command::Logs
                | Command::Run(..)
        ) {
            self.load_project(&args.project_args)?;
        }

        self.ctx.set_api_url(args.api_url);

        match args.cmd {
            Command::Deploy(deploy_args) => self.deploy(deploy_args).await,
            Command::Status => self.status().await,
            Command::Logs => self.logs().await,
            Command::Delete => self.delete().await,
            Command::Auth(auth_args) => self.auth(auth_args).await,
            Command::Login(login_args) => self.login(login_args).await,
            Command::Run(run_args) => self.local_run(run_args).await,
        }
    }

    pub fn load_project(&mut self, project_args: &ProjectArgs) -> Result<()> {
        trace!("loading project arguments: {project_args:?}");
        self.ctx.load_local(project_args)
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

    async fn logs(&self) -> Result<()> {
        client::logs(
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to get logs of deployment")
    }

    async fn local_run(&self, run_args: RunArgs) -> Result<()> {
        trace!("starting a local run for a service: {run_args:?}");

        let buf = Box::new(stdout());
        let working_directory = self.ctx.working_directory();

        trace!("building project");
        println!(
            "{:>12} {}",
            "Building".bold().green(),
            working_directory.display()
        );
        let so_path = build_crate(working_directory, buf)?;
        let loader = Loader::from_so_file(so_path)?;

        let mut factory = LocalFactory {};
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), run_args.port);
        let deployment_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        trace!("loading project");
        println!(
            "\n{:>12} {} on http://{}",
            "Starting".bold().green(),
            self.ctx.project_name(),
            addr
        );
        let (handle, so) = loader.load(&mut factory, addr, tx, deployment_id).await?;

        tokio::spawn(async move {
            while let Some(log) = rx.recv().await {
                print::log(log.datetime, log.item);
            }
        });

        handle.await??;

        tokio::spawn(async move {
            trace!("closing so file");
            so.close().unwrap();
        });

        Ok(())
    }

    async fn deploy(&self, args: DeployArgs) -> Result<()> {
        self.run_tests(args.no_test)?;
        self.check_lib_version().await?;

        let package_file = self
            .run_cargo_package(args.allow_dirty)
            .context("failed to package cargo project")?;

        let key = self.ctx.api_key()?;

        client::deploy(
            package_file,
            self.ctx.api_url(),
            key,
            self.ctx.project_name(),
        )
        .and_then(|_| {
            client::secrets(
                self.ctx.api_url(),
                key,
                self.ctx.project_name(),
                self.ctx.secrets(),
            )
        })
        .await
        .context("failed to deploy cargo project")
    }

    async fn check_lib_version(&self) -> Result<()> {
        let cargo_tree = PCmd::new("cargo")
            .args(["tree", "--prefix", "none"])
            .output()
            .map_err(|err| match err.kind() {
                // Just says 'program not found' otherwise
                io::ErrorKind::NotFound => {
                    io::Error::new(io::ErrorKind::NotFound, "Cargo not found")
                }
                _ => err,
            })?;

        let stdout = String::from_utf8_lossy(&cargo_tree.stdout);
        let mut service_version = String::new();

        stdout
            .as_ref()
            .lines()
            // Strips out trailing things like "(*)"
            .map(|line| match line.find('(') {
                Some(index) => &line[..index - 1],
                None => line,
            })
            .filter(|line| line.contains("shuttle-service"))
            .for_each(|dep| {
                service_version = dep.split_whitespace().skip(1).take(1).collect();
                service_version = service_version.replace("v", "");
            });

        let service_semver = Version::parse(&service_version)?;
        let mut server_version = String::new();

        client::version(self.ctx.api_url())
            .await
            .map(|response| server_version = response)?;

        let server_semver = Version::parse(&server_version)?;

        if service_semver.minor < server_semver.minor {
            return Err(anyhow!(
                "Update your shuttle-version to {}",
                &server_version,
            ));
        }
        return Ok(());
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

    fn run_tests(&self, no_test: bool) -> Result<()> {
        if no_test {
            return Ok(());
        }

        let config = cargo::util::config::Config::default()?;
        let working_directory = self.ctx.working_directory();
        let path = working_directory.join("Cargo.toml");

        let compile_options = CompileOptions::new(&config, CompileMode::Test).unwrap();
        let ws = Workspace::new(&path, &config)?;
        let opts = TestOptions {
            compile_opts: compile_options,
            no_run: false,
            no_fail_fast: false,
        };

        let test_failures = cargo::ops::run_tests(&ws, &opts, &[])?;
        match test_failures {
            None => Ok(()),
            Some(_) => Err(anyhow!(
                "Some tests failed. To ignore all tests, pass the `--no-test` flag"
            )),
        }
    }
}
