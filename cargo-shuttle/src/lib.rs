mod args;
mod client;
pub mod config;
mod init;
mod provisioner_server;
mod suggestions;

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fs::{read_to_string, File};
use std::io::stdout;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;

use shuttle_common::{
    claims::{ClaimService, InjectPropagation},
    constants::{API_URL_DEFAULT, EXECUTABLE_DIRNAME, STORAGE_DIRNAME},
    deployment::{DEPLOYER_END_MESSAGES_BAD, DEPLOYER_END_MESSAGES_GOOD},
    models::{
        deployment::{
            get_deployments_table, DeploymentRequest, CREATE_SERVICE_BODY_LIMIT,
            GIT_STRINGS_MAX_LENGTH,
        },
        project::{self, DEFAULT_IDLE_MINUTES},
        resource::get_resources_table,
        secret,
    },
    project::ProjectName,
    resource, semvers_are_compatible, ApiKey, LogItem, VersionInfo,
};
use shuttle_proto::runtime::{
    self, runtime_client::RuntimeClient, LoadRequest, StartRequest, StopRequest,
};
use shuttle_service::{
    builder::{build_workspace, BuiltService},
    Environment,
};

use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::Message;
use clap::{parser::ValueSource, CommandFactory, FromArgMatches};
use clap_complete::{generate, Shell};
use config::RequestContext;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Password};
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{StreamExt, TryFutureExt};
use git2::{Repository, StatusOptions};
use globset::{Glob, GlobSetBuilder};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use indicatif::ProgressBar;
use indoc::{formatdoc, printdoc};
use std::fmt::Write as FmtWrite;
use strum::IntoEnumIterator;
use tar::Builder;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::task::JoinHandle;
use tonic::transport::Channel;
use tonic::Status;
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

pub use crate::args::{Command, ProjectArgs, RunArgs, ShuttleArgs};
use crate::args::{
    DeployArgs, DeploymentCommand, InitArgs, LoginArgs, LogoutArgs, ProjectCommand,
    ProjectStartArgs, ResourceCommand, EXAMPLES_REPO,
};
use crate::client::Client;
use crate::provisioner_server::LocalProvisioner;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
const SHUTTLE_LOGIN_URL: &str = "https://console.shuttle.rs/new-project";
const SHUTTLE_GH_ISSUE_URL: &str = "https://github.com/shuttle-hq/shuttle/issues/new/choose";
const SHUTTLE_CLI_DOCS_URL: &str = "https://docs.shuttle.rs/getting-started/shuttle-commands";
const SHUTTLE_IDLE_DOCS_URL: &str = "https://docs.shuttle.rs/getting-started/idle-projects";

pub struct Shuttle {
    ctx: RequestContext,
    client: Option<Client>,
    version_info: Option<VersionInfo>,
    version_warnings: Vec<String>,
}

impl Shuttle {
    pub fn new() -> Result<Self> {
        let ctx = RequestContext::load_global()?;
        Ok(Self {
            ctx,
            client: None,
            version_info: None,
            version_warnings: vec![],
        })
    }

    pub async fn parse_args_and_run(self) -> Result<CommandOutcome> {
        // A hack to see if the PATH arg of the init command was explicitly given
        let matches = ShuttleArgs::command().get_matches();
        let args = ShuttleArgs::from_arg_matches(&matches)
            .expect("args to already be parsed successfully");
        let provided_path_to_init =
            matches
                .subcommand_matches("init")
                .is_some_and(|init_matches| {
                    init_matches.value_source("path") == Some(ValueSource::CommandLine)
                });

        self.run(args, provided_path_to_init).await
    }

    fn find_available_port(run_args: &mut RunArgs, services_len: usize) {
        let default_port = run_args.port;
        'outer: for port in (run_args.port..=std::u16::MAX).step_by(services_len.max(10)) {
            for inner_port in port..(port + services_len as u16) {
                if !portpicker::is_free_tcp(inner_port) {
                    continue 'outer;
                }
            }
            run_args.port = port;
            break;
        }

        if run_args.port != default_port
            && !Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Port {} is already in use. Would you like to continue on port {}?",
                    default_port, run_args.port
                ))
                .default(true)
                .interact()
                .unwrap()
        {
            exit(0);
        }
    }

    pub async fn run(
        mut self,
        args: ShuttleArgs,
        provided_path_to_init: bool,
    ) -> Result<CommandOutcome> {
        if let Some(ref url) = args.api_url {
            if url != API_URL_DEFAULT {
                println!("INFO: Targetting non-standard API: {url}");
            }
            if url.ends_with('/') {
                eprintln!("WARNING: API URL is probably incorrect. Ends with '/': {url}");
            }
        }
        self.ctx.set_api_url(args.api_url);

        // All commands that need to know which project is being handled
        if matches!(
            args.cmd,
            Command::Deploy(..)
                | Command::Deployment(..)
                | Command::Resource(..)
                | Command::Project(
                    // ProjectCommand::List does not need to know which project we are in
                    ProjectCommand::Start { .. }
                        | ProjectCommand::Stop { .. }
                        | ProjectCommand::Restart { .. }
                        | ProjectCommand::Status { .. }
                        | ProjectCommand::Delete
                )
                | Command::Stop
                | Command::Clean
                | Command::Secrets
                | Command::Status
                | Command::Logs { .. }
                | Command::Run(..)
        ) {
            self.load_project(&args.project_args)?;
        }

        // All commands that call the API
        if matches!(
            args.cmd,
            Command::Init(..)
                | Command::Deploy(..)
                | Command::Status
                | Command::Logs { .. }
                | Command::Logout(..)
                | Command::Deployment(..)
                | Command::Resource(..)
                | Command::Stop
                | Command::Clean
                | Command::Secrets
                | Command::Project(..)
        ) {
            let mut client = Client::new(self.ctx.api_url());
            if !matches!(args.cmd, Command::Init(..)) {
                // init command will handle this by itself (log in and set key) if there is no key yet
                client.set_api_key(self.ctx.api_key()?);
            }
            self.client = Some(client);
            self.check_api_versions().await?;
        }

        let res = match args.cmd {
            Command::Init(init_args) => {
                self.init(init_args, args.project_args, provided_path_to_init)
                    .await
            }
            Command::Generate { shell, output } => self.complete(shell, output),
            Command::Login(login_args) => self.login(login_args).await,
            Command::Logout(logout_args) => self.logout(logout_args).await,
            Command::Feedback => self.feedback(),
            Command::Run(run_args) => self.local_run(run_args).await,
            Command::Deploy(deploy_args) => self.deploy(deploy_args).await,
            Command::Status => self.status().await,
            Command::Logs { id, latest, follow } => self.logs(id, latest, follow).await,
            Command::Deployment(DeploymentCommand::List { page, limit }) => {
                self.deployments_list(page, limit).await
            }
            Command::Deployment(DeploymentCommand::Status { id }) => self.deployment_get(id).await,
            Command::Resource(ResourceCommand::List) => self.resources_list().await,
            Command::Stop => self.stop().await,
            Command::Clean => self.clean().await,
            Command::Secrets => self.secrets().await,
            Command::Resource(ResourceCommand::Delete { resource_type }) => {
                self.resource_delete(&resource_type).await
            }
            Command::Project(ProjectCommand::Start(ProjectStartArgs { idle_minutes })) => {
                self.project_create(idle_minutes).await
            }
            Command::Project(ProjectCommand::Restart(ProjectStartArgs { idle_minutes })) => {
                self.project_recreate(idle_minutes).await
            }
            Command::Project(ProjectCommand::Status { follow }) => {
                self.project_status(follow).await
            }
            Command::Project(ProjectCommand::List { page, limit }) => {
                self.projects_list(page, limit).await
            }
            Command::Project(ProjectCommand::Stop) => self.project_stop().await,
            Command::Project(ProjectCommand::Delete) => self.project_delete().await,
        };

        for w in self.version_warnings {
            println!("{w}");
        }

        res
    }

    async fn check_api_versions(&mut self) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        debug!("Checking API versions");
        if let Ok(versions) = client.get_api_versions().await {
            debug!("Got API versions: {versions:?}");
            self.version_info = Some(versions);

            // check cargo-shuttle version
            // should always be a valid semver
            let my_version = &semver::Version::from_str(VERSION).unwrap();
            let latest_version = &self.version_info.as_ref().unwrap().cargo_shuttle;
            if my_version != latest_version {
                let newer_version_exists = my_version < latest_version;
                let string = if semvers_are_compatible(my_version, latest_version) {
                    newer_version_exists.then(|| {
                        format!("Info: A newer version of cargo-shuttle exists ({latest_version}).")
                    })
                    // Having a newer but compatible version does not show warning
                } else {
                    newer_version_exists.then(||
                        formatdoc! {"
                            Warning:
                                A newer version of cargo-shuttle exists ({latest_version}).
                                It is recommended to upgrade.
                                Refer to the upgrading docs: https://docs.shuttle.rs/configuration/shuttle-versions#upgrading-shuttle-version"
                        }
                    ).or_else(||
                        Some(formatdoc! {"
                            Warning:
                                Your version of cargo-shuttle ({my_version}) is newer than what the API expects ({latest_version}).
                                This means a new release is likely underway!
                                Unexpected behavior can occur until the API is updated."
                        })
                    )
                };
                if let Some(s) = string {
                    self.version_warnings.push(s.yellow().to_string());
                }
            }
        } else {
            debug!("Failed to get API version info");
        }

        Ok(())
    }

    /// Log in, initialize a project and potentially create the Shuttle environment for it.
    ///
    /// If project name, template, and path are passed as arguments, it will run without any extra
    /// interaction.
    async fn init(
        &mut self,
        args: InitArgs,
        mut project_args: ProjectArgs,
        provided_path_to_init: bool,
    ) -> Result<CommandOutcome> {
        // Turns the template or git args (if present) to a repo+folder.
        let git_templates = args.git_template()?;

        let unauthorized = self.ctx.api_key().is_err() && args.login_args.api_key.is_none();

        let interactive = project_args.name.is_none()
            || git_templates.is_none()
            || !provided_path_to_init
            || unauthorized;

        let theme = ColorfulTheme::default();

        // 1. Log in (if not logged in yet)
        if let Ok(api_key) = self.ctx.api_key() {
            let login_args = LoginArgs {
                api_key: Some(api_key.as_ref().to_string()),
            };

            self.login(login_args).await?;
        } else if interactive {
            println!("First, let's log in to your Shuttle account.");
            self.login(args.login_args.clone()).await?;
            println!();
        } else if args.login_args.api_key.is_some() {
            self.login(args.login_args.clone()).await?;
        } else if args.create_env {
            bail!("Tried to login to create a Shuttle environment, but no API key was set.")
        }

        // 2. Ask for project name
        if project_args.name.is_none() {
            printdoc!(
                "
                What do you want to name your project?
                It will be hosted at ${{project_name}}.shuttleapp.rs, so choose something unique!
                "
            );
            let client = self.client.as_ref().unwrap();
            loop {
                // not using validate_with due to being blocking
                let p: ProjectName = Input::with_theme(&theme)
                    .with_prompt("Project name")
                    .interact()?;
                match client.check_project_name(&p).await {
                    Ok(true) => {
                        println!("{} {}", "Project name already taken:".red(), p);
                        println!("{}", "Try a different name.".yellow());
                    }
                    Ok(false) => {
                        project_args.name = Some(p);
                        break;
                    }
                    Err(_) => {
                        project_args.name = Some(p);
                        println!(
                            "{}",
                            "Failed to check if project name is available.".yellow()
                        );
                        break;
                    }
                }
            }
            println!();
        }

        // 3. Confirm the project directory
        let path = if interactive {
            let path = args
                .path
                .to_str()
                .context("path arg should always be set")?;

            println!("Where should we create this project?");
            let directory_str: String = Input::with_theme(&theme)
                .with_prompt("Directory")
                .default(path.to_owned())
                .interact()?;
            println!();

            let path = args::parse_init_path(OsString::from(directory_str))?;

            if std::fs::read_dir(&path)
                .expect("init dir to exist and list entries")
                .count()
                > 0
                && !Confirm::with_theme(&theme)
                    .with_prompt("Target directory is not empty. Are you sure?")
                    .default(true)
                    .interact()?
            {
                return Ok(CommandOutcome::Ok);
            }

            path
        } else {
            args.path.clone()
        };

        // 4. Ask for the framework
        let template = match git_templates {
            Some(git_templates) => git_templates,
            None => {
                println!(
                    "Shuttle works with a range of web frameworks. Which one do you want to use?"
                );
                let frameworks = args::InitTemplateArg::iter().collect::<Vec<_>>();
                let index = FuzzySelect::with_theme(&theme)
                    .with_prompt("Framework")
                    .items(&frameworks)
                    .default(0)
                    .interact()?;
                println!();
                frameworks[index].template()
            }
        };

        let serenity_idle_hint = if let Some(s) = template.subfolder.as_ref() {
            s.contains("serenity") || s.contains("poise")
        } else {
            false
        };

        // 5. Initialize locally
        init::generate_project(
            path.clone(),
            project_args
                .name
                .as_ref()
                .expect("to have a project name provided"),
            template,
        )?;
        println!();

        printdoc!(
            "
            Hint: Check the examples repo for a full list of templates:
                  {EXAMPLES_REPO}
            Hint: You can also use `cargo shuttle init --from` to clone templates.
                  See {SHUTTLE_CLI_DOCS_URL}
                  or run `cargo shuttle init --help`
            "
        );
        println!();

        // 6. Confirm that the user wants to create the project environment on Shuttle
        let should_create_environment = if !interactive {
            args.create_env
        } else if args.create_env {
            true
        } else {
            let should_create = Confirm::with_theme(&theme)
                .with_prompt(format!(
                    r#"Claim the project name "{}" by starting a project container on Shuttle?"#,
                    project_args
                        .name
                        .as_ref()
                        .expect("to have a project name provided")
                ))
                .default(true)
                .interact()?;
            if !should_create {
                println!(
                    "Note: The project name will not be claimed until \
                    you start the project with `cargo shuttle project start`."
                )
            }
            println!();
            should_create
        };

        if should_create_environment {
            // Set the project working directory path to the init path,
            // so `load_project` is ran with the correct project path
            project_args.working_directory = path.clone();

            self.load_project(&project_args)?;
            self.project_create(DEFAULT_IDLE_MINUTES).await?;
        }

        if std::env::current_dir().is_ok_and(|d| d != path) {
            println!("You can `cd` to the directory, then:");
        }
        println!("Run `cargo shuttle run` to run the app locally.");
        if !should_create_environment {
            println!(
                "Run `cargo shuttle project start` to create a project environment on Shuttle."
            );
            if serenity_idle_hint {
                printdoc!(
                    "
                    Hint: Discord bots might want to use `--idle-minutes 0` when starting the
                    project so that they don't go offline: {SHUTTLE_IDLE_DOCS_URL}
                    "
                );
            }
        }

        Ok(CommandOutcome::Ok)
    }

    pub fn load_project(&mut self, project_args: &ProjectArgs) -> Result<()> {
        trace!("loading project arguments: {project_args:?}");

        self.ctx.load_local(project_args)
    }

    /// Provide feedback on GitHub.
    fn feedback(&self) -> Result<CommandOutcome> {
        let _ = webbrowser::open(SHUTTLE_GH_ISSUE_URL);
        println!("If your browser did not open automatically, go to {SHUTTLE_GH_ISSUE_URL}");

        Ok(CommandOutcome::Ok)
    }

    /// Log in with the given API key or after prompting the user for one.
    async fn login(&mut self, login_args: LoginArgs) -> Result<CommandOutcome> {
        let api_key_str = match login_args.api_key {
            Some(api_key) => api_key,
            None => {
                let _ = webbrowser::open(SHUTTLE_LOGIN_URL);
                println!("If your browser did not automatically open, go to {SHUTTLE_LOGIN_URL}");

                Password::with_theme(&ColorfulTheme::default())
                    .with_prompt("API key")
                    .validate_with(|input: &String| ApiKey::parse(input).map(|_| ()))
                    .interact()?
            }
        };

        let api_key = ApiKey::parse(&api_key_str)?;

        self.ctx.set_api_key(api_key.clone())?;

        if let Some(client) = self.client.as_mut() {
            client.set_api_key(api_key);
        }

        Ok(CommandOutcome::Ok)
    }

    async fn logout(&mut self, logout_args: LogoutArgs) -> Result<CommandOutcome> {
        if logout_args.reset_api_key {
            self.reset_api_key()
                .await
                .map_err(suggestions::api_key::reset_api_key_failed)?;
            println!("Successfully reset the API key.");
            println!(" -> Go to {SHUTTLE_LOGIN_URL} to get a new one.\n");
        }
        self.ctx.clear_api_key()?;
        println!("Successfully logged out of shuttle.");

        Ok(CommandOutcome::Ok)
    }

    async fn reset_api_key(&self) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        client.reset_api_key().await.and_then(|res| {
            if res.status().is_success() {
                Ok(())
            } else {
                Err(anyhow!("Resetting API key failed."))
            }
        })
    }

    async fn stop(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let proj_name = self.ctx.project_name();
        let mut service = client
            .stop_service(proj_name)
            .await
            .map_err(suggestions::deployment::stop_deployment_failure)?;

        let progress_bar = create_spinner();
        loop {
            let Some(ref deployment) = service.deployment else {
                break;
            };

            if let shuttle_common::deployment::State::Stopped = deployment.state {
                break;
            }

            progress_bar.set_message(format!("Stopping {}", deployment.id));
            service = client.get_service(proj_name).await?;
        }
        progress_bar.finish_and_clear();

        println!("{}\n{}", "Successfully stopped service".bold(), service);
        println!("Run `cargo shuttle deploy` to re-deploy your service.");

        Ok(CommandOutcome::Ok)
    }

    fn complete(&self, shell: Shell, output: Option<PathBuf>) -> Result<CommandOutcome> {
        let name = env!("CARGO_PKG_NAME");
        let mut app = Command::command();
        match output {
            Some(v) => generate(shell, &mut app, name, &mut File::create(v)?),
            None => generate(shell, &mut app, name, &mut stdout()),
        };

        Ok(CommandOutcome::Ok)
    }

    async fn status(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let summary = client.get_service(self.ctx.project_name()).await?;

        println!("{summary}");

        Ok(CommandOutcome::Ok)
    }

    async fn secrets(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let secrets = client
            .get_secrets(self.ctx.project_name())
            .await
            .map_err(suggestions::resources::get_secrets_failure)?;
        let table = secret::get_table(&secrets);

        println!("{table}");

        Ok(CommandOutcome::Ok)
    }

    async fn clean(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let lines = client
            .clean_project(self.ctx.project_name())
            .await
            .map_err(|err| {
                suggestions::project::project_request_failure(
                    err,
                    "Project clean failed",
                    true,
                    "cleaning your project or checking its status fail repeteadly",
                )
            })?;

        for line in lines {
            println!("{line}");
        }

        println!("Cleaning done!");

        Ok(CommandOutcome::Ok)
    }

    async fn logs(&self, id: Option<Uuid>, latest: bool, follow: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let id = if let Some(id) = id {
            id
        } else {
            let proj_name = self.ctx.project_name();

            if latest {
                // Find latest deployment (not always an active one)
                let deployments = client
                    .get_deployments(proj_name, 0, 1)
                    .await
                    .map_err(|err| {
                        suggestions::logs::get_logs_failure(
                            err,
                            "Fetching the latest deployment failed",
                        )
                    })?;
                let most_recent = deployments.first().context(format!(
                    "Could not find any deployments for '{proj_name}'. Try passing a deployment ID manually",
                ))?;

                most_recent.id
            } else if let Some(deployment) = client.get_service(proj_name).await?.deployment {
                // Active deployment
                deployment.id
            } else {
                bail!(
                    "Could not find a running deployment for '{proj_name}'. \
                    Try with '--latest', or pass a deployment ID manually"
                );
            }
        };

        if follow {
            let mut stream = client
                .get_logs_ws(self.ctx.project_name(), &id)
                .await
                .map_err(|err| {
                    suggestions::logs::get_logs_failure(err, "Connecting to the logs stream failed")
                })?;

            while let Some(Ok(msg)) = stream.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                    let log_item: shuttle_common::LogItem = serde_json::from_str(&line)
                        .context("Failed parsing logs. Is your cargo-shuttle outdated?")?;
                    println!("{log_item}")
                }
            }
        } else {
            let logs = client
                .get_logs(self.ctx.project_name(), &id)
                .await
                .map_err(|err| {
                    suggestions::logs::get_logs_failure(err, "Fetching the deployment failed")
                })?;

            for log in logs.into_iter() {
                println!("{log}");
            }
        }

        Ok(CommandOutcome::Ok)
    }

    async fn deployments_list(&self, page: u32, limit: u32) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        if limit == 0 {
            println!();
            return Ok(CommandOutcome::Ok);
        }

        let proj_name = self.ctx.project_name();
        let deployments = client
            .get_deployments(proj_name, page, limit)
            .await
            .map_err(suggestions::deployment::get_deployments_list_failure)?;
        let table = get_deployments_table(&deployments, proj_name.as_str(), page);

        println!("{table}");
        println!("Run `cargo shuttle logs <id>` to get logs for a given deployment.");

        Ok(CommandOutcome::Ok)
    }

    async fn deployment_get(&self, deployment_id: Uuid) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let deployment = client
            .get_deployment_details(self.ctx.project_name(), &deployment_id)
            .await
            .map_err(suggestions::deployment::get_deployment_status_failure)?;

        println!("{deployment}");

        Ok(CommandOutcome::Ok)
    }

    async fn resources_list(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let resources = client
            .get_service_resources(self.ctx.project_name())
            .await
            .map_err(suggestions::resources::get_service_resources_failure)?;
        let table = get_resources_table(&resources, self.ctx.project_name().as_str());

        println!("{table}");

        Ok(CommandOutcome::Ok)
    }

    async fn resource_delete(&self, resource_type: &resource::Type) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        println!(
            "{}",
            formatdoc!(
                "
            WARNING:
                Are you sure you want to delete this project's {}?
                This action is permanent.",
                resource_type
            )
            .bold()
            .red()
        );
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure?")
            .default(false)
            .interact()
            .unwrap()
        {
            return Ok(CommandOutcome::Ok);
        }

        client
            .delete_service_resource(self.ctx.project_name(), resource_type)
            .await?;

        println!("Deleted resource {resource_type}");
        println!(
            "{}",
            formatdoc! {"
                Note:
                    Remember to remove the resource annotation from your #[shuttle_runtime::main] function.
                    Otherwise, it will be provisioned again during the next deployment."
            }
            .yellow(),
        );

        Ok(CommandOutcome::Ok)
    }

    async fn spin_local_runtime(
        run_args: &RunArgs,
        service: &BuiltService,
        provisioner_server: &JoinHandle<Result<(), tonic::transport::Error>>,
        idx: u16,
        provisioner_port: u16,
    ) -> Result<
        Option<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )>,
    > {
        let crate_directory = service.crate_directory();
        let secrets_path = if crate_directory.join("Secrets.dev.toml").exists() {
            crate_directory.join("Secrets.dev.toml")
        } else {
            crate_directory.join("Secrets.toml")
        };
        trace!("Loading secrets from {}", secrets_path.display());

        let secrets: HashMap<String, String> = if let Ok(secrets_str) = read_to_string(secrets_path)
        {
            let secrets: HashMap<String, String> =
                secrets_str.parse::<toml::Value>()?.try_into()?;

            trace!(keys = ?secrets.keys(), "available secrets");

            secrets
        } else {
            trace!("No secrets were loaded");
            Default::default()
        };

        let runtime_executable = if service.is_wasm {
            let runtime_path = home::cargo_home()
                .expect("failed to find cargo home dir")
                .join("bin/shuttle-next");

            println!("Installing shuttle-next runtime. This can take a while...");

            if cfg!(debug_assertions) {
                // Canonicalized path to shuttle-runtime for dev to work on windows

                let path = dunce::canonicalize(format!("{MANIFEST_DIR}/../runtime"))
                    .expect("path to shuttle-runtime does not exist or is invalid");

                std::process::Command::new("cargo")
                    .arg("install")
                    .arg("shuttle-runtime")
                    .arg("--path")
                    .arg(path)
                    .arg("--bin")
                    .arg("shuttle-next")
                    .arg("--features")
                    .arg("next")
                    .output()
                    .expect("failed to install the shuttle runtime");
            } else {
                // If the version of cargo-shuttle is different from shuttle-runtime,
                // or it isn't installed, try to install shuttle-runtime from crates.io.
                if let Err(err) = check_version(&runtime_path) {
                    warn!("{}", err);

                    trace!("installing shuttle-runtime");
                    std::process::Command::new("cargo")
                        .arg("install")
                        .arg("shuttle-runtime")
                        .arg("--bin")
                        .arg("shuttle-next")
                        .arg("--features")
                        .arg("next")
                        .output()
                        .expect("failed to install the shuttle runtime");
                };
            };

            runtime_path
        } else {
            trace!(path = ?service.executable_path, "using alpha runtime");
            if let Err(err) = check_version(&service.executable_path) {
                warn!("{}", err);
                if let Some(mismatch) = err.downcast_ref::<VersionMismatchError>() {
                    println!("Warning: {}.", mismatch);
                    if mismatch.shuttle_runtime > mismatch.cargo_shuttle {
                        // The runtime is newer than cargo-shuttle so we
                        // should help the user to update cargo-shuttle.
                        println!(
                            "[HINT]: You should update cargo-shuttle. \
                            Check out the installation docs for how to update: \
                            https://docs.shuttle.rs/getting-started/installation"
                        );
                    } else {
                        println!(
                            "[HINT]: A newer version of shuttle-runtime is available. \
                            Change its version to {} in this project's Cargo.toml to update it.",
                            mismatch.cargo_shuttle
                        );
                    }
                }
            }
            service.executable_path.clone()
        };

        // Child process and gRPC client for sending requests to it
        let (mut runtime, mut runtime_client) = runtime::start(
            service.is_wasm,
            Environment::Local,
            &format!("http://localhost:{provisioner_port}"),
            None,
            portpicker::pick_unused_port().expect("unable to find available port for gRPC server"),
            runtime_executable,
            service.workspace_path.as_path(),
        )
        .await
        .map_err(|err| {
            provisioner_server.abort();
            err
        })?;

        let service_name = service.service_name()?;
        let deployment_id: Uuid = Default::default();

        // Clones to send to spawn
        let service_name_clone = service_name.clone().to_string();

        let child_stdout = runtime
            .stdout
            .take()
            .context("child process did not have a handle to stdout")?;
        let mut reader = BufReader::new(child_stdout).lines();
        tokio::spawn(async move {
            while let Some(line) = reader.next_line().await.unwrap() {
                let log_item = LogItem::new(
                    deployment_id,
                    shuttle_common::log::Backend::Runtime(service_name_clone.clone()),
                    line,
                );
                println!("{log_item}");
            }
        });

        let load_request = tonic::Request::new(LoadRequest {
            path: service
                .executable_path
                .clone()
                .into_os_string()
                .into_string()
                .expect("to convert path to string"),
            service_name: service_name.to_string(),
            resources: Default::default(),
            secrets,
        });

        trace!("loading service");
        let response = runtime_client
            .load(load_request)
            .or_else(|err| async {
                provisioner_server.abort();
                runtime.kill().await?;
                Err(err)
            })
            .await?
            .into_inner();

        if !response.success {
            error!(error = response.message, "failed to load your service");
            return Ok(None);
        }

        let resources = response
            .resources
            .into_iter()
            .map(resource::Response::from_bytes)
            .collect();

        println!("{}", get_resources_table(&resources, service_name.as_str()));

        let addr = SocketAddr::new(
            if run_args.external {
                Ipv4Addr::UNSPECIFIED // 0.0.0.0
            } else {
                Ipv4Addr::LOCALHOST // 127.0.0.1
            }
            .into(),
            run_args.port + idx,
        );

        println!(
            "    {} {} on http://{}\n",
            "Starting".bold().green(),
            service_name,
            addr
        );

        let start_request = StartRequest {
            ip: addr.to_string(),
        };

        trace!(?start_request, "starting service");
        let response = runtime_client
            .start(tonic::Request::new(start_request))
            .or_else(|err| async {
                provisioner_server.abort();
                runtime.kill().await?;
                Err(err)
            })
            .await?
            .into_inner();

        trace!(response = ?response,  "client response: ");
        Ok(Some((runtime, runtime_client)))
    }

    async fn stop_runtime(
        runtime: &mut Child,
        runtime_client: &mut RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    ) -> Result<(), Status> {
        let stop_request = StopRequest {};
        trace!(?stop_request, "stopping service");
        let response = runtime_client
            .stop(tonic::Request::new(stop_request))
            .or_else(|err| async {
                runtime.kill().await?;
                trace!(status = ?err, "killed the runtime by force because stopping it errored out");
                Err(err)
            })
            .await?
            .into_inner();
        trace!(response = ?response,  "client stop response: ");
        Ok(())
    }

    async fn add_runtime_info(
        runtime: Option<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )>,
        existing_runtimes: &mut Vec<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )>,
        extra_servers: &[&JoinHandle<Result<(), tonic::transport::Error>>],
    ) -> Result<(), Status> {
        match runtime {
            Some(inner) => {
                trace!("Adding runtime PID: {:?}", inner.0.id());
                existing_runtimes.push(inner);
            }
            None => {
                trace!("Runtime error: No runtime process. Crashed during startup?");
                for server in extra_servers {
                    server.abort();
                }

                for rt_info in existing_runtimes {
                    let mut errored_out = false;
                    // Stopping all runtimes gracefully first, but if this errors out the function kills the runtime forcefully.
                    Shuttle::stop_runtime(&mut rt_info.0, &mut rt_info.1)
                        .await
                        .unwrap_or_else(|_| {
                            errored_out = true;
                        });

                    // If the runtime stopping is successful, we still need to kill it forcefully because we exit outside the loop
                    // and destructors will not be guaranteed to run.
                    if !errored_out {
                        rt_info.0.kill().await?;
                    }
                }
                exit(1);
            }
        };
        Ok(())
    }

    async fn pre_local_run(&self, run_args: &RunArgs) -> Result<Vec<BuiltService>> {
        trace!("starting a local run for a service: {run_args:?}");

        let (tx, rx): (crossbeam_channel::Sender<Message>, _) = crossbeam_channel::bounded(0);
        tokio::task::spawn_blocking(move || {
            while let Ok(message) = rx.recv() {
                match message {
                    Message::TextLine(line) => println!("{line}"),
                    message => {
                        trace!("skipping cargo line: {message:?}")
                    }
                }
            }
        });

        let working_directory = self.ctx.working_directory();

        trace!("building project");
        println!(
            "{} {}",
            "    Building".bold().green(),
            working_directory.display()
        );

        // Compile all the alpha or shuttle-next services in the workspace.
        build_workspace(working_directory, run_args.release, tx, false).await
    }

    async fn setup_local_provisioner(
    ) -> Result<(JoinHandle<Result<(), tonic::transport::Error>>, u16)> {
        let provisioner = LocalProvisioner::new()?;
        let provisioner_port =
            portpicker::pick_unused_port().expect("unable to find available port for provisioner");
        let provisioner_server = provisioner.start(SocketAddr::new(
            Ipv4Addr::LOCALHOST.into(),
            provisioner_port,
        ));

        Ok((provisioner_server, provisioner_port))
    }

    #[cfg(target_family = "unix")]
    async fn local_run(&self, mut run_args: RunArgs) -> Result<CommandOutcome> {
        let services = self.pre_local_run(&run_args).await?;
        let (provisioner_server, provisioner_port) = Shuttle::setup_local_provisioner().await?;
        let mut sigterm_notif =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Can not get the SIGTERM signal receptor");
        let mut sigint_notif =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("Can not get the SIGINT signal receptor");

        // Start all the services.
        let mut runtimes: Vec<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )> = Vec::new();

        Shuttle::find_available_port(&mut run_args, services.len());

        let mut signal_received = false;
        for (i, service) in services.iter().enumerate() {
            // We must cover the case of starting multiple workspace services and receiving a signal in parallel.
            // This must stop all the existing runtimes and creating new ones.
            signal_received = tokio::select! {
                res = Shuttle::spin_local_runtime(&run_args, service, &provisioner_server, i as u16, provisioner_port) => {
                    match res {
                        Ok(runtime) => {
                            Shuttle::add_runtime_info(runtime, &mut runtimes, &[&provisioner_server]).await?;
                        },
                        Err(e) => println!("Runtime error: {e:?}"),
                    }
                    false
                },
                _ = sigterm_notif.recv() => {
                    println!(
                        "cargo-shuttle received SIGTERM. Killing all the runtimes..."
                    );
                    true
                },
                _ = sigint_notif.recv() => {
                    println!(
                        "cargo-shuttle received SIGINT. Killing all the runtimes..."
                    );
                    true
                }
            };

            if signal_received {
                break;
            }
        }

        // If prior signal received is set to true we must stop all the existing runtimes and
        // exit the `local_run`.
        if signal_received {
            provisioner_server.abort();
            for (mut rt, mut rt_client) in runtimes {
                Shuttle::stop_runtime(&mut rt, &mut rt_client)
                    .await
                    .unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
            }
            return Ok(CommandOutcome::Ok);
        }

        // If no signal was received during runtimes initialization, then we must handle each runtime until
        // completion and handle the signals during this time.
        for (mut rt, mut rt_client) in runtimes {
            // If we received a signal while waiting for any runtime we must stop the rest and exit
            // the waiting loop.
            if signal_received {
                Shuttle::stop_runtime(&mut rt, &mut rt_client)
                    .await
                    .unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
                continue;
            }

            // Receiving a signal will stop the current runtime we're waiting for.
            signal_received = tokio::select! {
                res = rt.wait() => {
                    println!(
                        "a service future completed with exit status: {:?}",
                        res.unwrap().code()
                    );
                    false
                },
                _ = sigterm_notif.recv() => {
                    println!(
                        "cargo-shuttle received SIGTERM. Killing all the runtimes..."
                    );
                    provisioner_server.abort();
                    Shuttle::stop_runtime(&mut rt, &mut rt_client).await.unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
                    true
                },
                _ = sigint_notif.recv() => {
                    println!(
                        "cargo-shuttle received SIGINT. Killing all the runtimes..."
                    );
                    provisioner_server.abort();
                    Shuttle::stop_runtime(&mut rt, &mut rt_client).await.unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
                    true
                }
            };
        }

        println!(
            "Run `cargo shuttle project start` to create a project environment on Shuttle.\n\
             Run `cargo shuttle deploy` to deploy your Shuttle service."
        );

        Ok(CommandOutcome::Ok)
    }

    #[cfg(target_family = "windows")]
    async fn handle_signals() -> bool {
        let mut ctrl_break_notif = tokio::signal::windows::ctrl_break()
            .expect("Can not get the CtrlBreak signal receptor");
        let mut ctrl_c_notif =
            tokio::signal::windows::ctrl_c().expect("Can not get the CtrlC signal receptor");
        let mut ctrl_close_notif = tokio::signal::windows::ctrl_close()
            .expect("Can not get the CtrlClose signal receptor");
        let mut ctrl_logoff_notif = tokio::signal::windows::ctrl_logoff()
            .expect("Can not get the CtrlLogoff signal receptor");
        let mut ctrl_shutdown_notif = tokio::signal::windows::ctrl_shutdown()
            .expect("Can not get the CtrlShutdown signal receptor");

        tokio::select! {
            _ = ctrl_break_notif.recv() => {
                println!("cargo-shuttle received ctrl-break.");
                true
            },
            _ = ctrl_c_notif.recv() => {
                println!("cargo-shuttle received ctrl-c.");
                true
            },
            _ = ctrl_close_notif.recv() => {
                println!("cargo-shuttle received ctrl-close.");
                true
            },
            _ = ctrl_logoff_notif.recv() => {
                println!("cargo-shuttle received ctrl-logoff.");
                true
            },
            _ = ctrl_shutdown_notif.recv() => {
                println!("cargo-shuttle received ctrl-shutdown.");
                true
            }
            else => {
                false
            }
        }
    }

    #[cfg(target_family = "windows")]
    async fn local_run(&self, mut run_args: RunArgs) -> Result<CommandOutcome> {
        let services = self.pre_local_run(&run_args).await?;
        let (provisioner_server, provisioner_port) = Shuttle::setup_local_provisioner().await?;

        // Start all the services.
        let mut runtimes: Vec<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )> = Vec::new();

        Shuttle::find_available_port(&mut run_args, services.len());

        let mut signal_received = false;
        for (i, service) in services.iter().enumerate() {
            signal_received = tokio::select! {
                res = Shuttle::spin_local_runtime(&run_args, service, &provisioner_server, i as u16, provisioner_port) => {
                    Shuttle::add_runtime_info(res.unwrap(), &mut runtimes, &[&provisioner_server]).await?;
                    false
                },
                _ = Shuttle::handle_signals() => {
                    println!(
                        "Killing all the runtimes..."
                    );
                    true
                }
            };

            if signal_received {
                break;
            }
        }

        // If prior signal received is set to true we must stop all the existing runtimes and
        // exit the `local_run`.
        if signal_received {
            provisioner_server.abort();
            for (mut rt, mut rt_client) in runtimes {
                Shuttle::stop_runtime(&mut rt, &mut rt_client)
                    .await
                    .unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
            }
            return Ok(CommandOutcome::Ok);
        }

        // If no signal was received during runtimes initialization, then we must handle each runtime until
        // completion and handle the signals during this time.
        for (mut rt, mut rt_client) in runtimes {
            // If we received a signal while waiting for any runtime we must stop the rest and exit
            // the waiting loop.
            if signal_received {
                Shuttle::stop_runtime(&mut rt, &mut rt_client)
                    .await
                    .unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
                continue;
            }

            // Receiving a signal will stop the current runtime we're waiting for.
            signal_received = tokio::select! {
                res = rt.wait() => {
                    println!(
                        "a service future completed with exit status: {:?}",
                        res.unwrap().code()
                    );
                    false
                },
                _ = Shuttle::handle_signals() => {
                    println!(
                        "Killing all the runtimes..."
                    );
                    provisioner_server.abort();
                    Shuttle::stop_runtime(&mut rt, &mut rt_client).await.unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
                    true
                }
            };
        }

        println!(
            "Run `cargo shuttle project start` to create a project environment on Shuttle.\n\
             Run `cargo shuttle deploy` to deploy your Shuttle service."
        );

        Ok(CommandOutcome::Ok)
    }

    async fn deploy(&mut self, args: DeployArgs) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let working_directory = self.ctx.working_directory();

        let mut deployment_req: DeploymentRequest = DeploymentRequest {
            no_test: args.no_test,
            ..Default::default()
        };

        if let Ok(repo) = Repository::discover(working_directory) {
            let repo_path = repo
                .workdir()
                .context("getting working directory of repository")?;
            let repo_path = dunce::canonicalize(repo_path)?;
            trace!(?repo_path, "found git repository");

            let dirty = self.is_dirty(&repo);
            if !args.allow_dirty && dirty.is_err() {
                bail!(dirty.unwrap_err());
            }
            deployment_req.git_dirty = Some(dirty.is_err());

            if let Ok(head) = repo.head() {
                // This is typically the name of the current branch
                // It is "HEAD" when head detached, for example when a tag is checked out
                deployment_req.git_branch = head
                    .shorthand()
                    .map(|s| s.chars().take(GIT_STRINGS_MAX_LENGTH).collect());
                if let Ok(commit) = head.peel_to_commit() {
                    deployment_req.git_commit_id = Some(commit.id().to_string());
                    // Summary is None if error or invalid utf-8
                    deployment_req.git_commit_msg = commit
                        .summary()
                        .map(|s| s.chars().take(GIT_STRINGS_MAX_LENGTH).collect());
                }
            }
        }

        deployment_req.data = self.make_archive()?;
        if deployment_req.data.len() > CREATE_SERVICE_BODY_LIMIT {
            bail!(
                r#"The project is too large - the limit is {} MB. \
                Your project archive is {:.1} MB. \
                Run with `RUST_LOG="cargo_shuttle=debug"` to see which files are being packed."#,
                CREATE_SERVICE_BODY_LIMIT / 1_000_000,
                deployment_req.data.len() as f32 / 1_000_000f32,
            );
        }

        let deployment = client
            .deploy(self.ctx.project_name(), deployment_req)
            .await
            .map_err(suggestions::deploy::deploy_request_failure)?;

        let mut stream = client
            .get_logs_ws(self.ctx.project_name(), &deployment.id)
            .await
            .map_err(|err| {
                suggestions::deploy::deployment_setup_failure(
                    err,
                    "Connecting to the deployment logs failed",
                )
            })?;

        let mut deployer_version_checked = false;
        let mut runtime_version_checked = false;
        loop {
            let message = stream.next().await;
            if let Some(Ok(msg)) = message {
                if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                    let log_item: shuttle_common::LogItem =
                        serde_json::from_str(&line).expect("to parse log line");

                    println!("{log_item}");

                    // Detect versions of deployer and runtime, and print warnings of outdated.
                    if !deployer_version_checked
                        && self.version_info.is_some()
                        && log_item.line.contains("Deployer version: ")
                    {
                        deployer_version_checked = true;
                        let my_version = &log_item
                            .line
                            .split_once("Deployer version: ")
                            .unwrap()
                            .1
                            .parse::<semver::Version>()
                            .context("parsing deployer version in log stream")?;
                        let latest_version = &self.version_info.as_ref().unwrap().deployer;
                        if latest_version > my_version {
                            self.version_warnings.push(
                                formatdoc! {"
                                    Warning:
                                        A newer version of shuttle-deployer is available ({latest_version}).
                                        Use `cargo shuttle project restart` to upgrade."
                                }
                                .yellow()
                                .to_string(),
                            )
                        }
                    }
                    if !runtime_version_checked
                        && self.version_info.is_some()
                        && log_item
                            .line
                            .contains("shuttle-runtime executable started (version ")
                    {
                        runtime_version_checked = true;
                        let my_version = &log_item
                            .line
                            .split_once("shuttle-runtime executable started (version ")
                            .unwrap()
                            .1
                            .split_once(')')
                            .unwrap()
                            .0
                            .parse::<semver::Version>()
                            .context("parsing runtime version in log stream")?;
                        let latest_version = &self.version_info.as_ref().unwrap().runtime;
                        if latest_version > my_version {
                            self.version_warnings.push(
                                formatdoc! {"
                                    Warning:
                                        A newer version of shuttle-runtime is available ({latest_version}).
                                        Update it and any other shuttle dependencies in Cargo.toml."
                                }
                                .yellow()
                                .to_string(),
                            )
                        }
                    }

                    // Determine when to stop listening to the log stream
                    if DEPLOYER_END_MESSAGES_BAD
                        .iter()
                        .any(|m| log_item.line.contains(m))
                    {
                        println!();
                        println!("{}", "Deployment crashed".red());
                        println!();
                        println!("Run the following for more details");
                        println!();
                        println!("cargo shuttle logs {}", &deployment.id);

                        return Ok(CommandOutcome::DeploymentFailure);
                    }
                    if DEPLOYER_END_MESSAGES_GOOD
                        .iter()
                        .any(|m| log_item.line.contains(m))
                    {
                        debug!("received end message, breaking deployment stream");
                        break;
                    }
                }
            } else {
                eprintln!("--- Reconnecting websockets logging ---");
                // A wait time short enough for not much state to have changed, long enough that
                // the terminal isn't completely spammed
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                stream = client
                    .get_logs_ws(self.ctx.project_name(), &deployment.id)
                    .await
                    .map_err(|err| {
                        suggestions::deploy::deployment_setup_failure(
                            err,
                            "Connecting to the deployment logs failed",
                        )
                    })?;
            }
        }

        // Temporary fix.
        // TODO: Make get_service_summary endpoint wait for a bit and see if it entered Running/Crashed state.
        // Note: Will otherwise be possible when health checks are supported
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let deployment = client
            .get_deployment_details(self.ctx.project_name(), &deployment.id)
            .await
            .map_err(|err| {
                suggestions::deploy::deployment_setup_failure(
                    err,
                    "Assessing deployment state failed",
                )
            })?;

        // A deployment will only exist if there is currently one in the running state
        if deployment.state != shuttle_common::deployment::State::Running {
            println!("{}", "Deployment has not entered the running state".red());
            println!();

            match deployment.state {
                shuttle_common::deployment::State::Stopped => {
                    println!("State: Stopped - Deployment was running, but has been stopped by the user.")
                }
                shuttle_common::deployment::State::Completed => {
                    println!("State: Completed - Deployment was running, but stopped running all by itself.")
                }
                shuttle_common::deployment::State::Unknown => {
                    println!("State: Unknown - Deployment was in an unknown state. We never expect this state and entering this state should be considered a bug.")
                }
                shuttle_common::deployment::State::Crashed => {
                    println!(
                        "{}",
                        "State: Crashed - Deployment crashed after startup.".red()
                    );
                }
                state => {
                    debug!("deployment logs stream received state: {state} when it expected to receive running state");
                    println!(
                    "Deployment entered an unexpected state - Please create a ticket to report this."
                );
                }
            }

            println!();
            println!("Run the following for more details");
            println!();
            println!("cargo shuttle logs {}", &deployment.id);

            return Ok(CommandOutcome::DeploymentFailure);
        }

        let service = client.get_service(self.ctx.project_name()).await?;
        let resources = client
            .get_service_resources(self.ctx.project_name())
            .await?;
        let resources = get_resources_table(&resources, self.ctx.project_name().as_str());

        println!("{resources}{service}");

        Ok(CommandOutcome::Ok)
    }

    async fn project_create(&self, idle_minutes: u64) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let config = project::Config { idle_minutes };

        self.wait_with_spinner(
            &[
                project::State::Ready,
                project::State::Errored {
                    message: Default::default(),
                },
            ],
            client.create_project(self.ctx.project_name(), config),
            self.ctx.project_name(),
            client,
        )
        .await
        .map_err(|err| {
            suggestions::project::project_request_failure(
                err,
                "Project creation failed",
                true,
                "the project creation or retrieving the status fails repeteadly",
            )
        })?;

        if idle_minutes > 0 {
            let idle_msg = format!(
                "Your project will sleep if it is idle for {} minutes.",
                idle_minutes
            );
            println!("{}", idle_msg.yellow());
            println!("To change the idle time refer to the docs: {SHUTTLE_IDLE_DOCS_URL}");
            println!();
        }

        println!("Run `cargo shuttle deploy --allow-dirty` to deploy your Shuttle service.");

        Ok(CommandOutcome::Ok)
    }

    async fn project_recreate(&self, idle_minutes: u64) -> Result<CommandOutcome> {
        self.project_stop()
            .await
            .map_err(suggestions::project::project_restart_failure)?;
        self.project_create(idle_minutes)
            .await
            .map_err(suggestions::project::project_restart_failure)?;

        Ok(CommandOutcome::Ok)
    }

    async fn projects_list(&self, page: u32, limit: u32) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        if limit == 0 {
            println!();
            return Ok(CommandOutcome::Ok);
        }

        let projects = client.get_projects_list(page, limit).await.map_err(|err| {
            suggestions::project::project_request_failure(
                err,
                "Getting projects list failed",
                false,
                "getting the projects list fails repeteadly",
            )
        })?;
        let projects_table = project::get_table(&projects, page);

        println!("{projects_table}");

        Ok(CommandOutcome::Ok)
    }

    async fn project_status(&self, follow: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        if follow {
            self.wait_with_spinner(
                &[
                    project::State::Ready,
                    project::State::Destroyed,
                    project::State::Errored {
                        message: Default::default(),
                    },
                ],
                client.get_project(self.ctx.project_name()),
                self.ctx.project_name(),
                client,
            )
            .await?;
        } else {
            let project = client
                .get_project(self.ctx.project_name())
                .await
                .map_err(|err| {
                    suggestions::project::project_request_failure(
                        err,
                        "Getting project status failed",
                        false,
                        "getting project status failed repeteadly",
                    )
                })?;
            println!(
                "{project}\nIdle minutes: {}",
                project
                    .idle_minutes
                    .map(|i| i.to_string())
                    .unwrap_or("<unknown>".to_owned())
            );
        }

        Ok(CommandOutcome::Ok)
    }

    async fn project_stop(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        self.wait_with_spinner(
            &[
                project::State::Destroyed,
                project::State::Errored {
                    message: Default::default(),
                },
            ],
            client.stop_project(self.ctx.project_name()),
            self.ctx.project_name(),
            client,
        )
        .await
        .map_err(|err| {
            suggestions::project::project_request_failure(
                err,
                "Project stop failed",
                true,
                "stopping the project or getting project status fails repeteadly",
            )
        })?;
        println!("Run `cargo shuttle project start` to recreate project environment on Shuttle.");

        Ok(CommandOutcome::Ok)
    }

    async fn project_delete(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        println!(
            "{}",
            formatdoc!(
                r#"
                WARNING:
                    Are you sure you want to delete "{}"?
                    This will...
                    - Delete all Secrets and Persist data in this project.
                    - Release the project name from your account.
                    This action is permanent."#,
                self.ctx.project_name()
            )
            .bold()
            .red()
        );
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure?")
            .default(false)
            .interact()
            .unwrap()
        {
            return Ok(CommandOutcome::Ok);
        }

        client
            .delete_project(self.ctx.project_name())
            .await
            .map_err(|err| {
                suggestions::project::project_request_failure(
                    err,
                    "Project delete failed",
                    true,
                    "deleting the project or getting project status fails repeteadly",
                )
            })?;

        Ok(CommandOutcome::Ok)
    }

    async fn wait_with_spinner<'a, Fut>(
        &self,
        states_to_check: &[project::State],
        fut: Fut,
        project_name: &'a ProjectName,
        client: &'a Client,
    ) -> Result<(), anyhow::Error>
    where
        Fut: std::future::Future<Output = Result<project::Response>> + 'a,
    {
        let mut project = fut.await?;

        let progress_bar = create_spinner();
        loop {
            if states_to_check.contains(&project.state) {
                break;
            }

            progress_bar.set_message(format!("{project}"));
            project = client.get_project(project_name).await?;

            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        }
        progress_bar.finish_and_clear();
        println!("{project}");
        Ok(())
    }

    fn make_archive(&self) -> Result<Vec<u8>> {
        let include_patterns = self.ctx.assets();
        let encoder = GzEncoder::new(Vec::new(), Compression::new(3));
        let mut tar = Builder::new(encoder);

        let working_directory = self.ctx.working_directory();

        //
        // Mixing include and exclude overrides messes up the .ignore and .gitignore etc,
        // therefore these "ignore" walk and the "include" walk are separate.
        //
        let mut entries = Vec::new();

        // Default excludes
        let ignore_overrides = OverrideBuilder::new(working_directory)
            .add("!.git/")
            .context("adding override `!.git/`")?
            .add("!target/")
            .context("adding override `!target/`")?
            // these should always be ignored when unpacked in deployment, so ignore them here as well
            .add(&format!("!{EXECUTABLE_DIRNAME}/"))
            .context(format!("adding override `!{EXECUTABLE_DIRNAME}/`"))?
            .add(&format!("!{STORAGE_DIRNAME}/"))
            .context(format!("adding override `!{STORAGE_DIRNAME}/`"))?
            .build()
            .context("building archive override rules")?;
        for r in WalkBuilder::new(working_directory)
            .hidden(false)
            .overrides(ignore_overrides)
            .build()
        {
            entries.push(r.context("list dir entry")?.into_path())
        }

        let mut globs = GlobSetBuilder::new();

        // Always include secrets
        globs.add(Glob::new("**/Secrets.toml").unwrap());

        // User provided includes
        if let Some(rules) = include_patterns {
            for r in rules {
                globs.add(Glob::new(r.as_str()).context(format!("parsing glob pattern {:?}", r))?);
            }
        }

        // Find the files
        let globs = globs.build().context("glob glob")?;
        for entry in walkdir::WalkDir::new(working_directory) {
            let path = entry.context("list dir")?.into_path();
            if globs.is_match(
                path.strip_prefix(working_directory)
                    .context("strip prefix of path")?,
            ) {
                entries.push(path);
            }
        }

        let mut archive_files = BTreeMap::new();
        for path in entries {
            // It's not possible to add a directory to an archive
            // and symlinks == chaos
            if path.is_dir() || path.is_symlink() {
                trace!("Skipping {:?}", path);
                continue;
            }

            let name = path
                .strip_prefix(working_directory.parent().context("get parent dir")?)
                .context("strip prefix of path")?
                .to_owned();

            archive_files.insert(path, name);
        }

        if archive_files.is_empty() {
            error!("No files included in upload. Aborting...");
            bail!("No files included in upload.");
        }

        // Append all the entries to the archive.
        for (k, v) in archive_files {
            debug!("Packing {k:?}");
            tar.append_path_with_name(k, v)?;
        }

        let encoder = tar.into_inner().context("get encoder from tar archive")?;
        let bytes = encoder.finish().context("finish up encoder")?;
        debug!("Archive size: {} bytes", bytes.len());

        Ok(bytes)
    }

    fn is_dirty(&self, repo: &Repository) -> Result<()> {
        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        let statuses = repo
            .statuses(Some(&mut status_options))
            .context("getting status of repository files")?;

        if !statuses.is_empty() {
            let mut error = format!(
                "{} files in the working directory contain changes that were not yet committed into git:\n",
                statuses.len()
            );

            for status in statuses.iter() {
                trace!(
                    path = status.path(),
                    status = ?status.status(),
                    "found file with updates"
                );

                let rel_path = status.path().context("getting path of changed file")?;

                writeln!(error, "{rel_path}").expect("to append error");
            }

            writeln!(error).expect("to append error");
            writeln!(error, "To proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag").expect("to append error");

            bail!(error);
        }

        Ok(())
    }
}

fn check_version(runtime_path: &Path) -> Result<()> {
    // should always be a valid semver
    let my_version = semver::Version::from_str(VERSION).unwrap();

    if !runtime_path.try_exists()? {
        bail!("shuttle-runtime is not installed");
    }

    // Get runtime version from shuttle-runtime cli
    let runtime_version = std::process::Command::new(runtime_path)
        .arg("--version")
        .output()
        .context("failed to check the shuttle-runtime version")?
        .stdout;

    // Parse the version, splitting the version from the name and
    // and pass it to `to_semver()`.
    let runtime_version = semver::Version::from_str(
        std::str::from_utf8(&runtime_version)
            .expect("shuttle-runtime version should be valid utf8")
            .split_once(' ')
            .expect("shuttle-runtime version should be in the `name version` format")
            .1
            .trim(),
    )
    .context("failed to convert user's runtime version to semver")?;

    if semvers_are_compatible(&my_version, &runtime_version) {
        Ok(())
    } else {
        Err(VersionMismatchError {
            shuttle_runtime: runtime_version,
            cargo_shuttle: my_version,
        })
        .context("shuttle-runtime and cargo-shuttle have incompatible versions")
    }
}

#[derive(Debug)]
struct VersionMismatchError {
    shuttle_runtime: semver::Version,
    cargo_shuttle: semver::Version,
}

impl std::fmt::Display for VersionMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "shuttle-runtime {} and cargo-shuttle {} are incompatible",
            self.shuttle_runtime, self.cargo_shuttle
        )
    }
}

impl std::error::Error for VersionMismatchError {}

fn create_spinner() -> ProgressBar {
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

    pb
}

#[derive(PartialEq)]
pub enum CommandOutcome {
    Ok,
    DeploymentFailure,
}

#[cfg(test)]
mod tests {
    use flate2::read::GzDecoder;
    use shuttle_common::project::ProjectName;
    use tar::Archive;

    use crate::args::ProjectArgs;
    use crate::Shuttle;
    use std::fs::{self, canonicalize};
    use std::path::PathBuf;
    use std::str::FromStr;

    pub fn path_from_workspace_root(path: &str) -> PathBuf {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path);

        dunce::canonicalize(path).unwrap()
    }

    fn get_archive_entries(project_args: ProjectArgs) -> Vec<String> {
        let mut shuttle = Shuttle::new().unwrap();
        shuttle.load_project(&project_args).unwrap();

        let archive = shuttle.make_archive().unwrap();

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
    fn make_archive_respect_rules() {
        let working_directory = canonicalize(path_from_workspace_root(
            "cargo-shuttle/tests/resources/archiving",
        ))
        .unwrap();

        fs::write(working_directory.join("Secrets.toml"), "KEY = 'value'").unwrap();
        fs::write(working_directory.join("Secrets.dev.toml"), "KEY = 'dev'").unwrap();
        fs::write(working_directory.join("asset2"), "").unwrap();
        fs::write(working_directory.join("asset4"), "").unwrap();
        fs::create_dir_all(working_directory.join("dist")).unwrap();
        fs::write(working_directory.join("dist").join("dist1"), "").unwrap();

        fs::create_dir_all(working_directory.join("target")).unwrap();
        fs::write(working_directory.join("target").join("binary"), b"12345").unwrap();

        let project_args = ProjectArgs {
            working_directory,
            name: Some(ProjectName::from_str("archiving-test").unwrap()),
        };
        let mut entries = get_archive_entries(project_args);
        entries.sort();

        assert_eq!(
            entries,
            vec![
                ".gitignore",
                ".ignore",
                "Cargo.toml",
                "Secrets.toml", // always included by default
                "Secrets.toml.example",
                "Shuttle.toml",
                "asset1", // normal file
                "asset2", // .gitignore'd, but included in Shuttle.toml
                // asset3 is .ignore'd
                "asset4",                // .gitignore'd, but un-ignored in .ignore
                "asset5",                // .ignore'd, but included in Shuttle.toml
                "dist/dist1",            // .gitignore'd, but included in Shuttle.toml
                "nested/static/nested1", // normal file
                // nested/static/nestedignore is .gitignore'd
                "src/main.rs",
            ]
        );
    }

    #[test]
    fn load_project_returns_proper_working_directory_in_project_args() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
        };

        let mut shuttle = Shuttle::new().unwrap();
        shuttle.load_project(&project_args).unwrap();

        assert_eq!(
            project_args.working_directory,
            path_from_workspace_root("examples/axum/hello-world/src")
        );
        assert_eq!(
            project_args.workspace_path().unwrap(),
            path_from_workspace_root("examples/axum/hello-world")
        );
    }
}
