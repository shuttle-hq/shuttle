mod args;
mod client;
pub mod config;
mod factory;
mod init;

use shuttle_common::project::ProjectName;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{read_to_string, File};
use std::io::stdout;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use args::AuthArgs;
pub use args::{Args, Command, DeployArgs, InitArgs, LoginArgs, ProjectArgs, RunArgs};
use cargo_metadata::Message;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use config::RequestContext;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Password};
use factory::LocalFactory;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::StreamExt;
use git2::{Repository, StatusOptions};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use shuttle_common::models::{project, secret};
use shuttle_service::loader::{build_crate, Loader};
use shuttle_service::Logger;
use std::fmt::Write;
use strum::IntoEnumIterator;
use tar::Builder;
use tokio::sync::mpsc;
use tracing::trace;
use uuid::Uuid;

use crate::args::{DeploymentCommand, ProjectCommand};
use crate::client::Client;

pub struct Shuttle {
    ctx: RequestContext,
}

impl Shuttle {
    pub fn new() -> Result<Self> {
        let ctx = RequestContext::load_global()?;
        Ok(Self { ctx })
    }

    pub async fn run(mut self, mut args: Args) -> Result<CommandOutcome> {
        trace!("running local client");
        if matches!(
            args.cmd,
            Command::Deploy(..)
                | Command::Deployment(..)
                | Command::Project(..)
                | Command::Delete
                | Command::Secrets
                | Command::Status
                | Command::Logs { .. }
                | Command::Run(..)
        ) {
            self.load_project(&mut args.project_args)?;
        }

        self.ctx.set_api_url(args.api_url);

        match args.cmd {
            Command::Init(init_args) => self.init(init_args, args.project_args).await,
            Command::Generate { shell, output } => self.complete(shell, output).await,
            Command::Login(login_args) => self.login(login_args).await,
            Command::Run(run_args) => self.local_run(run_args).await,
            need_client => {
                let mut client = Client::new(self.ctx.api_url());
                client.set_api_key(self.ctx.api_key()?);

                match need_client {
                    Command::Deploy(deploy_args) => {
                        return self.deploy(deploy_args, &client).await;
                    }
                    Command::Status => self.status(&client).await,
                    Command::Logs { id, follow } => self.logs(&client, id, follow).await,
                    Command::Deployment(DeploymentCommand::List) => {
                        self.deployments_list(&client).await
                    }
                    Command::Deployment(DeploymentCommand::Status { id }) => {
                        self.deployment_get(&client, id).await
                    }
                    Command::Delete => self.delete(&client).await,
                    Command::Secrets => self.secrets(&client).await,
                    Command::Auth(auth_args) => self.auth(auth_args, &client).await,
                    Command::Project(ProjectCommand::New) => self.project_create(&client).await,
                    Command::Project(ProjectCommand::Status { follow }) => {
                        self.project_status(&client, follow).await
                    }
                    Command::Project(ProjectCommand::Rm) => self.project_delete(&client).await,
                    _ => {
                        unreachable!("commands that don't need a client have already been matched")
                    }
                }
            }
        }
        .map(|_| CommandOutcome::Ok)
    }

    /// Log in, initialize a project and potentially create the Shuttle environment for it.
    ///
    /// If both a project name and framework are passed as arguments, it will run without any extra
    /// interaction.
    async fn init(&mut self, args: InitArgs, mut project_args: ProjectArgs) -> Result<()> {
        let interactive = project_args.name.is_none() || args.framework().is_none();

        let theme = ColorfulTheme::default();

        // 1. Log in (if not logged in yet)
        if self.ctx.api_key().is_err() {
            if interactive {
                println!("First, let's log in to your Shuttle account.");
                self.login(args.login_args.clone()).await?;
                println!();
            } else if args.new && args.login_args.api_key.is_some() {
                self.login(args.login_args.clone()).await?;
            } else {
                bail!("Tried to login to create a Shuttle environment, but no API key was set.")
            }
        }

        // 2. Ask for project name
        if project_args.name.is_none() {
            println!("How do you want to name your project? It will be hosted at ${{project_name}}.shuttleapp.rs.");
            // TODO: Check whether the project name is still available
            project_args.name = Some(
                Input::with_theme(&theme)
                    .with_prompt("Project name")
                    .interact()?,
            );
            println!();
        }

        // 3. Confirm the project directory
        let path = if interactive {
            println!("Where should we create this project?");
            let directory_str: String = Input::with_theme(&theme)
                .with_prompt("Directory")
                .default(".".to_owned())
                .interact()?;
            println!();
            args::parse_init_path(&OsString::from(directory_str))?
        } else {
            args.path.clone()
        };

        // 4. Ask for the framework
        let framework = match args.framework() {
            Some(framework) => framework,
            None => {
                println!(
                    "Shuttle works with a range of web frameworks. Which one do you want to use?"
                );
                let frameworks = init::Framework::iter().collect::<Vec<_>>();
                let index = FuzzySelect::with_theme(&theme)
                    .items(&frameworks)
                    .default(0)
                    .interact()?;
                println!();
                frameworks[index]
            }
        };

        // 5. Initialize locally
        init::cargo_init(path.clone())?;
        init::cargo_shuttle_init(path, framework)?;
        println!();

        // 6. Confirm that the user wants to create the project environment on Shuttle
        let should_create_environment = if !interactive {
            args.new
        } else if args.new {
            true
        } else {
            Confirm::with_theme(&theme)
                .with_prompt("Do you want to create the project environment on Shuttle?")
                .default(true)
                .interact()?
        };
        if should_create_environment {
            self.load_project(&mut project_args)?;
            let mut client = Client::new(self.ctx.api_url());
            client.set_api_key(self.ctx.api_key()?);
            self.project_create(&client).await?;
        }

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

    /// Log in with the given API key or after prompting the user for one.
    async fn login(&mut self, login_args: LoginArgs) -> Result<()> {
        let api_key_str = match login_args.api_key {
            Some(api_key) => api_key,
            None => {
                let url = "https://shuttle.rs/login";
                let _ = webbrowser::open(url);

                println!("If your browser did not automatically open, go to {url}");

                Password::with_theme(&ColorfulTheme::default())
                    .with_prompt("API key")
                    .interact()?
            }
        };

        let api_key = api_key_str.trim().parse()?;

        self.ctx.set_api_key(api_key)?;

        Ok(())
    }

    async fn auth(&mut self, auth_args: AuthArgs, client: &Client) -> Result<()> {
        let user = client.auth(auth_args.username).await?;

        self.ctx.set_api_key(user.key)?;

        println!("User authorized!!!");
        println!("Run `cargo shuttle init --help` next");

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

    async fn complete(&self, shell: Shell, output: Option<PathBuf>) -> Result<()> {
        let name = env!("CARGO_PKG_NAME");
        let mut app = Command::command();
        match output {
            Some(v) => generate(shell, &mut app, name, &mut File::create(v)?),
            None => generate(shell, &mut app, name, &mut stdout()),
        };

        Ok(())
    }

    async fn status(&self, client: &Client) -> Result<()> {
        let summary = client.get_service_summary(self.ctx.project_name()).await?;

        println!("{summary}");

        Ok(())
    }

    async fn secrets(&self, client: &Client) -> Result<()> {
        let secrets = client.get_secrets(self.ctx.project_name()).await?;
        let table = secret::get_table(&secrets);

        println!("{table}");

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
            let mut stream = client.get_logs_ws(self.ctx.project_name(), &id).await?;

            while let Some(Ok(msg)) = stream.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                    let log_item: shuttle_common::LogItem =
                        serde_json::from_str(&line).expect("to parse log line");
                    println!("{log_item}")
                }
            }
        } else {
            let logs = client.get_logs(self.ctx.project_name(), &id).await?;

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
        let deployment = client
            .get_deployment_details(self.ctx.project_name(), &deployment_id)
            .await?;

        println!("{deployment}");

        Ok(())
    }

    async fn local_run(&self, run_args: RunArgs) -> Result<()> {
        trace!("starting a local run for a service: {run_args:?}");

        let (tx, rx): (crossbeam_channel::Sender<Message>, _) = crossbeam_channel::bounded(0);
        tokio::task::spawn_blocking(move || {
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
        let id = Default::default();

        trace!("building project");
        println!(
            "{:>12} {}",
            "Building".bold().green(),
            working_directory.display()
        );
        let so_path = build_crate(id, working_directory, false, tx).await?;

        trace!("loading secrets");
        let secrets_path = working_directory.join("Secrets.toml");

        let secrets: BTreeMap<String, String> =
            if let Ok(secrets_str) = read_to_string(secrets_path) {
                let secrets: BTreeMap<String, String> =
                    secrets_str.parse::<toml::Value>()?.try_into()?;

                trace!(keys = ?secrets.keys(), "available secrets");

                secrets
            } else {
                trace!("no Secrets.toml was found");
                Default::default()
            };

        let loader = Loader::from_so_file(so_path)?;

        let mut factory = LocalFactory::new(
            self.ctx.project_name().clone(),
            secrets,
            working_directory.to_path_buf(),
        )?;
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), run_args.port);

        trace!("loading project");
        println!(
            "\n{:>12} {} on http://{}",
            "Starting".bold().green(),
            self.ctx.project_name(),
            addr
        );
        let (tx, mut rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(log) = rx.recv().await {
                println!("{log}");
            }
        });

        let logger = Logger::new(tx, id);
        let (handle, so) = loader.load(&mut factory, addr, logger).await?;

        handle.await??;

        tokio::task::spawn_blocking(move || {
            trace!("closing so file");
            so.close().unwrap();
        });

        Ok(())
    }

    async fn deploy(&self, args: DeployArgs, client: &Client) -> Result<CommandOutcome> {
        if !args.allow_dirty {
            self.is_dirty()?;
        }

        let data = self.make_archive()?;

        let deployment = client
            .deploy(data, self.ctx.project_name(), args.no_test)
            .await?;

        let mut stream = client
            .get_logs_ws(self.ctx.project_name(), &deployment.id)
            .await?;

        while let Some(Ok(msg)) = stream.next().await {
            if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                let log_item: shuttle_common::LogItem =
                    serde_json::from_str(&line).expect("to parse log line");

                match log_item.state {
                    shuttle_common::deployment::State::Queued
                    | shuttle_common::deployment::State::Building
                    | shuttle_common::deployment::State::Built
                    | shuttle_common::deployment::State::Loading => {
                        println!("{log_item}");
                    }
                    shuttle_common::deployment::State::Crashed => {
                        println!();
                        println!("{}", "Deployment crashed".red());
                        println!("Run the following for more details");
                        println!();
                        print!("cargo shuttle logs {}", deployment.id);
                        println!();

                        return Ok(CommandOutcome::DeploymentFailure);
                    }
                    shuttle_common::deployment::State::Running
                    | shuttle_common::deployment::State::Completed
                    | shuttle_common::deployment::State::Stopped
                    | shuttle_common::deployment::State::Unknown => break,
                }
            }
        }

        let service = client.get_service_summary(self.ctx.project_name()).await?;

        // A deployment will only exist if there is currently one in the running state
        if let Some(ref new_deployment) = service.deployment {
            println!("{service}");

            Ok(match new_deployment.state {
                shuttle_common::deployment::State::Crashed => CommandOutcome::DeploymentFailure,
                _ => CommandOutcome::Ok,
            })
        } else {
            println!("Deployment has not entered the running state");

            Ok(CommandOutcome::DeploymentFailure)
        }
    }

    async fn project_create(&self, client: &Client) -> Result<()> {
        self.wait_with_spinner(
            &[project::State::Ready, project::State::Errored],
            Client::create_project,
            self.ctx.project_name(),
            client,
        )
        .await?;

        Ok(())
    }

    async fn project_status(&self, client: &Client, follow: bool) -> Result<()> {
        match follow {
            true => {
                self.wait_with_spinner(
                    &[
                        project::State::Ready,
                        project::State::Destroyed,
                        project::State::Errored,
                    ],
                    Client::get_project,
                    self.ctx.project_name(),
                    client,
                )
                .await?;
            }
            false => {
                let project = client.get_project(self.ctx.project_name()).await?;
                println!("{project}");
            }
        }

        Ok(())
    }

    async fn wait_with_spinner<'a, F, Fut>(
        &self,
        states_to_check: &[project::State],
        f: F,
        project_name: &'a ProjectName,
        client: &'a Client,
    ) -> Result<(), anyhow::Error>
    where
        F: Fn(&'a Client, &'a ProjectName) -> Fut,
        Fut: std::future::Future<Output = Result<project::Response>> + 'a,
    {
        let mut project = f(client, project_name).await?;
        let pb = indicatif::ProgressBar::new_spinner();
        pb.enable_steady_tick(std::time::Duration::from_millis(350));
        pb.set_style(
            indicatif::ProgressStyle::with_template("{spinner:.orange} {msg}")
                .unwrap()
                .tick_strings(&[
                    "( ●    )",
                    "(  ●   )",
                    "(   ●  )",
                    "(    ● )",
                    "(     ●)",
                    "(    ● )",
                    "(   ●  )",
                    "(  ●   )",
                    "( ●    )",
                    "(●     )",
                    "(●●●●●●)",
                ]),
        );
        loop {
            if states_to_check.contains(&project.state) {
                break;
            }

            pb.set_message(format!("{project}"));
            project = client.get_project(project_name).await?;
        }
        pb.finish_with_message("Done");
        println!("{project}");
        Ok(())
    }

    async fn project_delete(&self, client: &Client) -> Result<()> {
        let project = client.delete_project(self.ctx.project_name()).await?;

        println!("{project}");

        Ok(())
    }

    fn make_archive(&self) -> Result<Vec<u8>> {
        let encoder = GzEncoder::new(Vec::new(), Compression::fast());
        let mut tar = Builder::new(encoder);

        let working_directory = self.ctx.working_directory();
        let base_directory = working_directory
            .parent()
            .context("get parent directory of crate")?;

        // Make sure the target folder is excluded at all times
        let overrides = OverrideBuilder::new(working_directory)
            .add("!target/")
            .context("add `!target/` override")?
            .build()
            .context("build an override")?;

        for dir_entry in WalkBuilder::new(working_directory)
            .hidden(false)
            .overrides(overrides)
            .build()
        {
            let dir_entry = dir_entry.context("get directory entry")?;

            // It's not possible to add a directory to an archive
            if dir_entry.file_type().context("get file type")?.is_dir() {
                continue;
            }

            let path = dir_entry
                .path()
                .strip_prefix(base_directory)
                .context("strip the base of the archive entry")?;

            tar.append_path_with_name(dir_entry.path(), path)
                .context("archive entry")?;
        }

        // Make sure to add any `Secrets.toml` files
        let secrets_path = self.ctx.working_directory().join("Secrets.toml");
        if secrets_path.exists() {
            tar.append_path_with_name(secrets_path, Path::new("shuttle").join("Secrets.toml"))?;
        }

        let encoder = tar.into_inner().context("get encoder from tar archive")?;
        let bytes = encoder.finish().context("finish up encoder")?;

        Ok(bytes)
    }

    fn is_dirty(&self) -> Result<()> {
        let working_directory = self.ctx.working_directory();
        if let Ok(repo) = Repository::discover(working_directory) {
            let repo_path = repo
                .workdir()
                .context("getting working directory of repository")?;

            trace!(?repo_path, "found git repository");

            let repo_rel_path = working_directory
                .strip_prefix(repo_path)
                .context("stripping repository path from working directory")?;

            trace!(
                ?repo_rel_path,
                "got working directory path relative to git repository"
            );

            let mut status_options = StatusOptions::new();
            status_options
                .pathspec(repo_rel_path)
                .include_untracked(true);

            let statuses = repo
                .statuses(Some(&mut status_options))
                .context("getting status of repository files")?;

            if !statuses.is_empty() {
                let mut error: String = format!("{} files in the working directory contain changes that were not yet committed into git:", statuses.len());
                writeln!(error).expect("to append error");

                for status in statuses.iter() {
                    trace!(
                        path = status.path(),
                        status = ?status.status(),
                        "found file with updates"
                    );

                    let path =
                        repo_path.join(status.path().context("getting path of changed file")?);
                    let rel_path = path
                        .strip_prefix(working_directory)
                        .expect("getting relative path of changed file")
                        .display();

                    writeln!(error, "{rel_path}").expect("to append error");
                }

                writeln!(error).expect("to append error");
                writeln!(error, "to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag").expect("to append error");

                return Err(anyhow::Error::msg(error));
            }
        }

        Ok(())
    }
}

pub enum CommandOutcome {
    Ok,
    DeploymentFailure,
}

#[cfg(test)]
mod tests {
    use flate2::read::GzDecoder;
    use shuttle_common::project::ProjectName;
    use tar::Archive;
    use tempfile::TempDir;

    use crate::args::ProjectArgs;
    use crate::Shuttle;
    use std::fs::{self, canonicalize};
    use std::path::PathBuf;
    use std::str::FromStr;

    fn path_from_workspace_root(path: &str) -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path)
    }

    fn get_archive_entries(mut project_args: ProjectArgs) -> Vec<String> {
        let mut shuttle = Shuttle::new().unwrap();
        shuttle.load_project(&mut project_args).unwrap();

        let archive = shuttle.make_archive().unwrap();

        // Make sure the Secrets.toml file is not initially present
        let tar = GzDecoder::new(&archive[..]);
        let mut archive = Archive::new(tar);

        archive
            .entries()
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .unwrap()
                    .components()
                    .skip(1)
                    .collect::<PathBuf>()
                    .display()
                    .to_string()
            })
            .collect()
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

        let mut shuttle = Shuttle::new().unwrap();
        Shuttle::load_project(&mut shuttle, &mut project_args).unwrap();

        assert_eq!(
            project_args.working_directory,
            path_from_workspace_root("examples/axum/hello-world/")
        );
    }

    #[test]
    fn make_archive_include_secrets() {
        let working_directory =
            canonicalize(path_from_workspace_root("examples/rocket/secrets")).unwrap();

        fs::write(
            working_directory.join("Secrets.toml"),
            "MY_API_KEY = 'the contents of my API key'",
        )
        .unwrap();

        let project_args = ProjectArgs {
            working_directory,
            name: None,
        };

        let mut entries = get_archive_entries(project_args);
        entries.sort();

        assert_eq!(
            entries,
            vec![
                ".gitignore",
                "Cargo.toml",
                "README.md",
                "Secrets.toml",
                "Secrets.toml.example",
                "Shuttle.toml",
                "src/lib.rs",
            ]
        );
    }

    #[test]
    fn make_archive_respect_ignore() {
        let tmp_dir = TempDir::new().unwrap();
        let working_directory = tmp_dir.path();

        fs::write(working_directory.join(".env"), "API_KEY = 'blabla'").unwrap();
        fs::write(working_directory.join(".ignore"), ".env").unwrap();
        fs::write(working_directory.join("Cargo.toml"), "[package]").unwrap();

        let project_args = ProjectArgs {
            working_directory: working_directory.to_path_buf(),
            name: Some(ProjectName::from_str("secret").unwrap()),
        };

        let mut entries = get_archive_entries(project_args);
        entries.sort();

        assert_eq!(entries, vec![".ignore", "Cargo.toml"]);
    }

    #[test]
    fn make_archive_ignore_target_folder() {
        let tmp_dir = TempDir::new().unwrap();
        let working_directory = tmp_dir.path();

        fs::create_dir_all(working_directory.join("target")).unwrap();
        fs::write(working_directory.join("target").join("binary"), "12345").unwrap();
        fs::write(working_directory.join("Cargo.toml"), "[package]").unwrap();

        let project_args = ProjectArgs {
            working_directory: working_directory.to_path_buf(),
            name: Some(ProjectName::from_str("exclude_target").unwrap()),
        };

        let mut entries = get_archive_entries(project_args);
        entries.sort();

        assert_eq!(entries, vec!["Cargo.toml"]);
    }
}
