mod args;
mod client;
pub mod config;
mod factory;
mod init;
mod logger;
mod print;

use std::fs::{read_to_string, File};
use std::io::Write;
use std::io::{self, stdout};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
pub use args::{Args, Command, DeployArgs, InitArgs, ProjectArgs, RunArgs};
use args::{AuthArgs, LoginArgs};
use cargo::core::resolver::CliFeatures;
use cargo::core::Workspace;
use cargo::ops::{PackageOpts, Packages};
use cargo_metadata::Message;
use config::RequestContext;
use crossterm::style::Stylize;
use factory::LocalFactory;
use futures::StreamExt;
use semver::{Version, VersionReq};
use shuttle_common::deployment;
use shuttle_service::loader::{build_crate, Loader};
use toml_edit::Document;
use tracing::trace;
use uuid::Uuid;

use crate::args::DeploymentCommand;
use crate::client::Client;
use crate::logger::Logger;

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

    pub async fn run(mut self, mut args: Args) -> Result<CommandOutcome> {
        trace!("running local client");
        if matches!(
            args.cmd,
            Command::Deploy(..)
                | Command::Deployment(..)
                | Command::Delete
                | Command::Status
                | Command::Logs { .. }
                | Command::Run(..)
        ) {
            self.load_project(&mut args.project_args)?;
        }

        self.ctx.set_api_url(args.api_url);

        let mut client = Client::new(self.ctx.api_url());
        client.set_api_key(self.ctx.api_key());

        match args.cmd {
            Command::Deploy(deploy_args) => {
                self.check_lib_version(args.project_args, &client).await?;
                return self.deploy(deploy_args, &client).await;
            }
            Command::Init(init_args) => self.init(init_args).await,
            Command::Status => self.status(&client).await,
            Command::Logs { id, follow } => self.logs(&client, id, follow).await,
            Command::Deployment(DeploymentCommand::List) => self.deployments_list(&client).await,
            Command::Deployment(DeploymentCommand::Status { id }) => {
                self.deployment_get(&client, id).await
            }
            Command::Delete => self.delete(&client).await,
            Command::Auth(auth_args) => self.auth(auth_args, &client).await,
            Command::Login(login_args) => self.login(login_args).await,
            Command::Run(run_args) => self.local_run(run_args).await,
        }
        .map(|_| CommandOutcome::Ok)
    }

    async fn init(&self, args: InitArgs) -> Result<()> {
        // Interface with cargo to initialize new lib package for shuttle
        let path = args.path.clone();
        init::cargo_init(path.clone())?;

        let framework = init::get_framework(&args);
        init::cargo_shuttle_init(path, framework)?;

        Ok(())
    }

    fn find_root_directory(dir: &Path) -> Option<PathBuf> {
        dir.ancestors()
            .find(|ancestor| ancestor.join("Cargo.toml").exists())
            .map(|path| path.to_path_buf())
    }

    pub fn load_project(&mut self, project_args: &mut ProjectArgs) -> Result<()> {
        trace!("loading project arguments: {project_args:?}");
        let root_directory_path = Self::find_root_directory(&project_args.working_directory);

        if let Some(working_directory) = root_directory_path {
            project_args.working_directory = working_directory;
        } else {
            return Err(anyhow!("Could not locate the root of a cargo project. Are you inside a cargo project? You can also use `--working-directory` to locate your cargo project."));
        }

        self.ctx.load_local(project_args)
    }

    async fn login(&mut self, login_args: LoginArgs) -> Result<()> {
        let api_key_str = login_args.api_key.unwrap_or_else(|| {
            let url = "https://shuttle.rs/login";

            let _ = webbrowser::open(url);

            println!("If your browser did not automatically open, go to {url}");
            print!("Enter Api Key: ");

            stdout().flush().unwrap();

            let mut input = String::new();

            io::stdin().read_line(&mut input).unwrap();

            input
        });

        let api_key = api_key_str.trim().parse()?;

        self.ctx.set_api_key(api_key)?;

        Ok(())
    }

    async fn auth(&mut self, auth_args: AuthArgs, client: &Client) -> Result<()> {
        let api_key = client.auth(auth_args.username).await?;

        self.ctx.set_api_key(api_key)?;

        Ok(())
    }

    async fn delete(&self, client: &Client) -> Result<()> {
        let service = client.delete_service(self.ctx.project_name()).await?;

        println!(
            r#"{}
{}"#,
            "Successfully deleted service".bold(),
            service
        );

        Ok(())
    }

    async fn status(&self, client: &Client) -> Result<()> {
        let summary = client.get_service_summary(self.ctx.project_name()).await?;

        println!("{summary}");

        Ok(())
    }

    async fn logs(&self, client: &Client, id: Option<Uuid>, follow: bool) -> Result<()> {
        let id = if let Some(id) = id {
            id
        } else {
            let summary = client.get_service_summary(self.ctx.project_name()).await?;

            if let Some(deployment) = summary.deployment {
                deployment.id
            } else {
                return Err(anyhow!("could not automatically find a running deployment for '{}'. Try passing a deployment ID manually", self.ctx.project_name()));
            }
        };

        if follow {
            let mut stream = client.get_runtime_logs_ws(&id).await?;

            while let Some(Ok(msg)) = stream.next().await {
                match msg {
                    tokio_tungstenite::tungstenite::Message::Text(line) => {
                        let log_item: shuttle_common::log::Item =
                            serde_json::from_str(&line).expect("to parse log line");
                        println!("{log_item}")
                    }
                    _ => {}
                }
            }
        } else {
            let logs = client.get_runtime_logs(&id).await?;

            for log in logs.into_iter() {
                println!("{log}");
            }
        }

        Ok(())
    }

    async fn deployments_list(&self, client: &Client) -> Result<()> {
        let details = client.get_service_details(self.ctx.project_name()).await?;

        println!("{details}");

        Ok(())
    }

    async fn deployment_get(&self, client: &Client, deployment_id: Uuid) -> Result<()> {
        let deployment = client.get_deployment_details(&deployment_id).await?;

        println!("{deployment}");

        Ok(())
    }

    async fn local_run(&self, run_args: RunArgs) -> Result<()> {
        trace!("starting a local run for a service: {run_args:?}");

        let (tx, rx): (crossbeam_channel::Sender<Message>, _) = crossbeam_channel::bounded(0);
        tokio::spawn(async move {
            while let Ok(message) = rx.recv() {
                match message {
                    Message::TextLine(line) => println!("{line}"),
                    Message::CompilerMessage(message) => {
                        if let Some(rendered) = message.message.rendered {
                            println!("{rendered}");
                        }
                    }
                    _ => {}
                }
            }
        });

        let working_directory = self.ctx.working_directory();

        trace!("building project");
        println!(
            "{:>12} {}",
            "Building".bold().green(),
            working_directory.display()
        );
        let so_path = build_crate(working_directory, tx).await?;

        let loader = Loader::from_so_file(so_path)?;

        let mut factory = LocalFactory::new(self.ctx.project_name().clone())?;
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), run_args.port);

        trace!("loading project");
        println!(
            "\n{:>12} {} on http://{}",
            "Starting".bold().green(),
            self.ctx.project_name(),
            addr
        );
        let logger = Box::new(Logger);
        let (handle, so) = loader.load(&mut factory, addr, logger).await?;

        handle.await??;

        tokio::spawn(async move {
            trace!("closing so file");
            so.close().unwrap();
        });

        Ok(())
    }

    async fn deploy(&self, args: DeployArgs, client: &Client) -> Result<CommandOutcome> {
        let package_file = self
            .run_cargo_package(args.allow_dirty)
            .context("failed to package cargo project")?;

        let deployment = client
            .deploy(package_file, self.ctx.project_name(), args.no_test)
            .await?;

        println!("");
        println!("{deployment}");

        let mut stream = client.get_build_logs_ws(&deployment.id).await?;

        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                tokio_tungstenite::tungstenite::Message::Text(line) => println!("{line}"),
                _ => {}
            }
        }

        let service = client.get_service_summary(self.ctx.project_name()).await?;

        if let Some(ref new_deployment) = service.deployment {
            if new_deployment.id != deployment.id {
                println!(
                    "Deployment has not entered the running state so kept previous deployment up"
                );

                return Ok(CommandOutcome::DeploymentFailure);
            }

            println!("{service}");

            let key = self.ctx.api_key().unwrap();
            client::secrets(
                self.ctx.api_url(),
                &key,
                self.ctx.project_name(),
                self.ctx.secrets(),
            )
            .await
            .context("failed to set up secrets for deployment")?;

            Ok(match new_deployment.state {
                deployment::State::Crashed => CommandOutcome::DeploymentFailure,
                _ => CommandOutcome::Ok,
            })
        } else {
            println!("Deployment has not entered the running state");

            Ok(CommandOutcome::DeploymentFailure)
        }
    }

    async fn check_lib_version(&self, project_args: ProjectArgs, client: &Client) -> Result<()> {
        let cargo_path = project_args.working_directory.join("Cargo.toml");
        let cargo_doc = read_to_string(cargo_path.clone())?.parse::<Document>()?;
        let current_shuttle_version = &cargo_doc["dependencies"]["shuttle-service"]["version"];
        let service_semver = match Version::parse(current_shuttle_version.as_str().unwrap()) {
            Ok(version) => version,
            Err(error) => return Err(anyhow!("Your shuttle-service version ({}) is invalid and should follow the MAJOR.MINOR.PATCH semantic versioning format. Error given: {:?}", current_shuttle_version.as_str().unwrap(), error.to_string())),
        };

        let server_version = client.get_shuttle_service_version().await?;

        let version_required = format!("{}.{}", server_version.major, server_version.minor);
        let server_semver = VersionReq::parse(&version_required)?;

        if server_semver.matches(&service_semver) {
            Ok(())
        } else {
            Err(anyhow!(
                "Your shuttle_service version is outdated. Update your shuttle_service version to {} and try to deploy again",
                &server_version,
            ))
        }
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
            keep_going: false,
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

pub enum CommandOutcome {
    Ok,
    DeploymentFailure,
}

#[cfg(test)]
mod tests {
    use crate::args::ProjectArgs;
    use crate::Shuttle;
    use std::path::PathBuf;

    fn path_from_workspace_root(path: &str) -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path)
    }

    #[test]
    fn find_root_directory_returns_proper_directory() {
        let working_directory = path_from_workspace_root("examples/axum/hello-world/src");

        let root_dir = Shuttle::find_root_directory(&working_directory).unwrap();

        assert_eq!(
            root_dir,
            path_from_workspace_root("examples/axum/hello-world/")
        );
    }

    #[test]
    fn load_project_returns_proper_working_directory_in_project_args() {
        let mut project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
        };

        let mut shuttle = Shuttle::new();
        Shuttle::load_project(&mut shuttle, &mut project_args).unwrap();

        assert_eq!(
            project_args.working_directory,
            path_from_workspace_root("examples/axum/hello-world/")
        );
    }
}
