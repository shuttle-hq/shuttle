mod args;
mod client;
pub mod config;
mod init;
mod provisioner_server;
mod suggestions;

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fmt::Write as FmtWrite;
use std::fs::{read_to_string, File};
use std::io::stdout;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use args::{ConfirmationArgs, GenerateCommand};
use clap::{parser::ValueSource, CommandFactory, FromArgMatches};
use clap_complete::{generate, Shell};
use clap_mangen::Man;
use config::RequestContext;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{StreamExt, TryFutureExt};
use git2::{Repository, StatusOptions};
use globset::{Glob, GlobSetBuilder};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use indicatif::ProgressBar;
use indoc::{formatdoc, printdoc};
use shuttle_common::{
    constants::{
        API_URL_DEFAULT, DEFAULT_IDLE_MINUTES, EXAMPLES_REPO, EXECUTABLE_DIRNAME,
        RESOURCE_SCHEMA_VERSION, SHUTTLE_GH_ISSUE_URL, SHUTTLE_IDLE_DOCS_URL,
        SHUTTLE_INSTALL_DOCS_URL, SHUTTLE_LOGIN_URL, STORAGE_DIRNAME, TEMPLATES_SCHEMA_VERSION,
    },
    deployment::{DEPLOYER_END_MESSAGES_BAD, DEPLOYER_END_MESSAGES_GOOD},
    models::{
        deployment::{
            get_deployments_table, DeploymentRequest, CREATE_SERVICE_BODY_LIMIT,
            GIT_STRINGS_MAX_LENGTH,
        },
        error::ApiError,
        project,
        resource::get_resource_tables,
    },
    resource::{self, ResourceInput, ShuttleResourceOutput},
    semvers_are_compatible,
    templates::TemplatesSchema,
    ApiKey, DatabaseResource, DbInput, LogItem, VersionInfo,
};
use shuttle_proto::{
    provisioner::{provisioner_server::Provisioner, DatabaseRequest},
    runtime::{self, LoadRequest, StartRequest, StopRequest},
};
use shuttle_service::{
    builder::{build_workspace, BuiltService},
    runner, Environment,
};
use strum::{EnumMessage, VariantArray};
use tar::Builder;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::time::{sleep, Duration};
use tonic::{Request, Status};
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

pub use crate::args::{Command, ProjectArgs, RunArgs, ShuttleArgs};
use crate::args::{
    DeployArgs, DeploymentCommand, InitArgs, LoginArgs, LogoutArgs, LogsArgs, ProjectCommand,
    ProjectStartArgs, ResourceCommand, TemplateLocation,
};
use crate::client::Client;
use crate::provisioner_server::LocalProvisioner;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Returns the args and whether the PATH arg of the init command was explicitly given
pub fn parse_args() -> (ShuttleArgs, bool) {
    let matches = ShuttleArgs::command().get_matches();
    let args =
        ShuttleArgs::from_arg_matches(&matches).expect("args to already be parsed successfully");
    let provided_path_to_init = matches
        .subcommand_matches("init")
        .is_some_and(|init_matches| {
            init_matches.value_source("path") == Some(ValueSource::CommandLine)
        });

    (args, provided_path_to_init)
}

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
                eprintln!("INFO: Targeting non-standard API: {url}");
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
                        | ProjectCommand::Delete { .. }
                )
                | Command::Stop
                | Command::Clean
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
                | Command::Project(..)
        ) {
            let mut client = Client::new(self.ctx.api_url());
            if !matches!(args.cmd, Command::Init(..)) {
                // init command will handle this by itself (log in and set key) if there is no key yet
                client.set_api_key(self.ctx.api_key()?);
            }
            self.client = Some(client);
            if !args.offline {
                self.check_api_versions().await?;
            }
        }

        let res = match args.cmd {
            Command::Init(init_args) => {
                self.init(
                    init_args,
                    args.project_args,
                    provided_path_to_init,
                    args.offline,
                )
                .await
            }
            Command::Generate(GenerateCommand::Manpage) => self.generate_manpage(),
            Command::Generate(GenerateCommand::Shell { shell, output }) => {
                self.complete(shell, output)
            }
            Command::Login(login_args) => self.login(login_args).await,
            Command::Logout(logout_args) => self.logout(logout_args).await,
            Command::Feedback => self.feedback(),
            Command::Run(run_args) => self.local_run(run_args).await,
            Command::Deploy(deploy_args) => self.deploy(deploy_args).await,
            Command::Status => self.status().await,
            Command::Logs(logs_args) => self.logs(logs_args).await,
            Command::Deployment(DeploymentCommand::List { page, limit, raw }) => {
                self.deployments_list(page, limit, raw).await
            }
            Command::Deployment(DeploymentCommand::Status { id }) => self.deployment_get(id).await,
            Command::Resource(ResourceCommand::List { raw, show_secrets }) => {
                self.resources_list(raw, show_secrets).await
            }
            Command::Stop => self.stop().await,
            Command::Clean => self.clean().await,
            Command::Resource(ResourceCommand::Delete {
                resource_type,
                confirmation: ConfirmationArgs { yes },
            }) => self.resource_delete(&resource_type, yes).await,
            Command::Project(ProjectCommand::Start(ProjectStartArgs { idle_minutes })) => {
                self.project_start(idle_minutes).await
            }
            Command::Project(ProjectCommand::Restart(ProjectStartArgs { idle_minutes })) => {
                self.project_restart(idle_minutes).await
            }
            Command::Project(ProjectCommand::Status { follow }) => {
                self.project_status(follow).await
            }
            Command::Project(ProjectCommand::List { page, limit, raw }) => {
                self.projects_list(page, limit, raw).await
            }
            Command::Project(ProjectCommand::Stop) => self.project_stop().await,
            Command::Project(ProjectCommand::Delete(ConfirmationArgs { yes })) => {
                self.project_delete(yes).await
            }
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
        offline: bool,
    ) -> Result<CommandOutcome> {
        // Turns the template or git args (if present) to a repo+folder.
        let git_template = args.git_template()?;
        let no_git = args.no_git;

        let unauthorized = self.ctx.api_key().is_err() && args.login_args.api_key.is_none();

        let needs_name = project_args.name.is_none();
        let needs_template = git_template.is_none();
        let needs_path = !provided_path_to_init;
        let needs_login = unauthorized;
        let interactive = needs_name || needs_template || needs_path || needs_login;

        let theme = ColorfulTheme::default();

        // 1. Log in (if not logged in yet)
        if let Ok(api_key) = self.ctx.api_key() {
            let login_args = LoginArgs {
                api_key: Some(api_key.as_ref().to_string()),
            };
            // TODO: this re-applies an already loaded API key
            self.login(login_args).await?;
        } else if needs_login {
            println!("First, let's log in to your Shuttle account.");
            self.login(args.login_args.clone()).await?;
            println!();
        } else if args.login_args.api_key.is_some() {
            self.login(args.login_args.clone()).await?;
        } else if args.create_env {
            bail!("Tried to login to create a Shuttle environment, but no API key was set.")
        }

        // 2. Ask for project name or validate the given one
        if needs_name {
            printdoc! {"
                What do you want to name your project?
                It will be hosted at ${{project_name}}.shuttleapp.rs, so choose something unique!
                "
            };
        }
        let mut prev_name: Option<String> = None;
        loop {
            // prompt if interactive
            let name: String = if let Some(name) = project_args.name.clone() {
                name
            } else {
                // not using `validate_with` due to being blocking.
                Input::with_theme(&theme)
                    .with_prompt("Project name")
                    .interact()?
            };
            let force_name = args.force_name
                || (needs_name && prev_name.as_ref().is_some_and(|prev| prev == &name));
            if force_name {
                project_args.name = Some(name);
                break;
            }
            // validate and take action based on result
            if self
                .check_project_name(&mut project_args, name.clone())
                .await
            {
                // success
                break;
            } else if needs_name {
                // try again
                println!(r#"Type the same name again to use "{}" anyways."#, name);
                prev_name = Some(name);
            } else {
                // don't continue if non-interactive
                bail!(
                    "Invalid or unavailable project name. Use `--force-name` to use this project name anyways."
                );
            }
        }
        if needs_name {
            println!();
        }

        // 3. Confirm the project directory
        let path = if needs_path {
            let path = args
                .path
                .join(project_args.name.as_ref().expect("name should be set"));

            loop {
                println!("Where should we create this project?");

                let directory_str: String = Input::with_theme(&theme)
                    .with_prompt("Directory")
                    .default(format!("{}", path.display()))
                    .interact()?;
                println!();

                let path = args::create_and_parse_path(OsString::from(directory_str))?;

                if std::fs::read_dir(&path)
                    .expect("init dir to exist and list entries")
                    .count()
                    > 0
                    && !Confirm::with_theme(&theme)
                        .with_prompt("Target directory is not empty. Are you sure?")
                        .default(true)
                        .interact()?
                {
                    println!();
                    continue;
                }

                break path;
            }
        } else {
            args.path.clone()
        };

        // 4. Ask for the template
        let template = match git_template {
            Some(git_template) => git_template,
            None => {
                // Try to present choices from our up-to-date examples.
                // Fall back to the internal (potentially outdated) list.
                let schema = if offline {
                    None
                } else {
                    get_templates_schema()
                        .await
                        .map_err(|e| {
                            error!(err = %e, "Failed to get templates");
                            println!(
                                "{}",
                                "Failed to look up template list. Falling back to internal list."
                                    .yellow()
                            )
                        })
                        .ok()
                        .and_then(|s| {
                            if s.version == TEMPLATES_SCHEMA_VERSION {
                                return Some(s);
                            }
                            println!(
                                "{}",
                                "Template list with incompatible version found. Consider updating cargo-shuttle. Falling back to internal list."
                                    .yellow()
                            );

                            None
                        })
                };
                if let Some(schema) = schema {
                    println!("What type of project template would you like to start from?");
                    let i = Select::with_theme(&theme)
                        .items(&[
                            "A Hello World app in a supported framework",
                            "Browse our full library of templates", // TODO(when templates page is live): Add link to it?
                        ])
                        .clear(false)
                        .default(0)
                        .interact()?;
                    println!();
                    if i == 0 {
                        // Use a Hello world starter
                        let mut starters = schema.starters.into_values().collect::<Vec<_>>();
                        starters.sort_by_key(|t| {
                            // Make the "No templates" appear last in the list
                            if t.title.starts_with("No") {
                                "zzz".to_owned()
                            } else {
                                t.title.clone()
                            }
                        });
                        let starter_strings = starters
                            .iter()
                            .map(|t| {
                                format!("{} - {}", t.title.clone().bold(), t.description.clone())
                            })
                            .collect::<Vec<_>>();
                        let index = Select::with_theme(&theme)
                            .with_prompt("Select template")
                            .items(&starter_strings)
                            .default(0)
                            .interact()?;
                        println!();
                        let path = starters[index]
                            .path
                            .clone()
                            .expect("starter to have a path");

                        TemplateLocation {
                            auto_path: EXAMPLES_REPO.into(),
                            subfolder: Some(path),
                        }
                    } else {
                        // Browse all non-starter templates
                        let mut templates = schema.templates.into_values().collect::<Vec<_>>();
                        templates.sort_by_key(|t| t.title.clone());
                        let template_strings = templates
                            .iter()
                            .map(|t| {
                                format!(
                                    "{} - {}{}",
                                    t.title.clone().bold(),
                                    t.description.clone(),
                                    t.tags
                                        .first()
                                        .map(|tag| format!(" ({tag})").dim().to_string())
                                        .unwrap_or_default(),
                                )
                            })
                            .collect::<Vec<_>>();
                        let index = Select::with_theme(&theme)
                            .with_prompt("Select template")
                            .items(&template_strings)
                            .default(0)
                            .interact()?;
                        println!();
                        let path = templates[index]
                            .path
                            .clone()
                            .expect("template to have a path");

                        TemplateLocation {
                            auto_path: EXAMPLES_REPO.into(),
                            subfolder: Some(path),
                        }
                    }
                } else {
                    println!("Shuttle works with many frameworks. Which one do you want to use?");
                    let frameworks = args::InitTemplateArg::VARIANTS;
                    let framework_strings = frameworks
                        .iter()
                        .map(|t| {
                            t.get_documentation()
                                .expect("all template variants to have docs")
                        })
                        .collect::<Vec<_>>();
                    let index = Select::with_theme(&theme)
                        .items(&framework_strings)
                        .default(0)
                        .interact()?;
                    println!();
                    frameworks[index].template()
                }
            }
        };

        let serenity_idle_hint = template
            .subfolder
            .as_ref()
            .is_some_and(|s| s.contains("serenity") || s.contains("poise"));

        // 5. Initialize locally
        init::generate_project(
            path.clone(),
            project_args
                .name
                .as_ref()
                .expect("to have a project name provided"),
            template,
            no_git,
        )?;
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
            self.project_start(DEFAULT_IDLE_MINUTES).await?;
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

    /// true -> success/neutral. false -> try again.
    async fn check_project_name(&self, project_args: &mut ProjectArgs, name: String) -> bool {
        let client = self.client.as_ref().unwrap();
        match client.check_project_name(&name).await {
            Ok(true) => {
                println!("{} {}", "Project name already taken:".red(), name);
                println!("{}", "Try a different name.".yellow());

                false
            }
            Ok(false) => {
                project_args.name = Some(name);

                true
            }
            Err(e) => {
                // If API error contains message regarding format of error name, print that error and prompt again
                if let Ok(api_error) = e.downcast::<ApiError>() {
                    // If the returned error string changes, this could break
                    if api_error.message.contains("Invalid project name") {
                        println!("{}", api_error.message.yellow());
                        println!("{}", "Try a different name.".yellow());
                        return false;
                    }
                }
                // Else, the API error was about something else.
                // Ignore and keep going to not prevent the flow of the init command.
                project_args.name = Some(name);
                println!(
                    "{}",
                    "Failed to check if project name is available.".yellow()
                );

                true
            }
        }
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
        let p = self.ctx.project_name();
        wait_with_spinner(|i, pb| async move {
            let service = if i == 0 {
                client.stop_service(p).await?
            } else {
                client.get_service(p).await?
            };

            let service_str = format!("{service}");
            let cleanup = move || {
                println!("{}", "Successfully stopped service".bold());
                println!("{service_str}");
                println!("Run `cargo shuttle deploy` to re-deploy your service.");
            };

            let Some(ref deployment) = service.deployment else {
                return Ok(Some(cleanup));
            };

            pb.set_message(format!("Stopping {}", deployment.id));

            if deployment.state == shuttle_common::deployment::State::Stopped {
                Ok(Some(cleanup))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(suggestions::deployment::stop_deployment_failure)?;

        Ok(CommandOutcome::Ok)
    }

    fn complete(&self, shell: Shell, output: Option<PathBuf>) -> Result<CommandOutcome> {
        let name = env!("CARGO_PKG_NAME");
        let mut app = Command::command();
        match output {
            Some(path) => generate(shell, &mut app, name, &mut File::create(path)?),
            None => generate(shell, &mut app, name, &mut stdout()),
        };
        Ok(CommandOutcome::Ok)
    }

    fn generate_manpage(&self) -> Result<CommandOutcome> {
        let app = ShuttleArgs::command();
        let output = std::io::stdout();
        let mut output_handle = output.lock();

        Man::new(app.clone()).render(&mut output_handle)?;

        for subcommand in app.get_subcommands() {
            let primary = Man::new(subcommand.clone());
            primary.render_name_section(&mut output_handle)?;
            primary.render_synopsis_section(&mut output_handle)?;
            primary.render_description_section(&mut output_handle)?;
            primary.render_options_section(&mut output_handle)?;
            // For example, `generate` has sub-commands `shell` and `manpage`
            if subcommand.has_subcommands() {
                primary.render_subcommands_section(&mut output_handle)?;
                for sb in subcommand.get_subcommands() {
                    let secondary = Man::new(sb.clone());
                    secondary.render_name_section(&mut output_handle)?;
                    secondary.render_synopsis_section(&mut output_handle)?;
                    secondary.render_description_section(&mut output_handle)?;
                    secondary.render_options_section(&mut output_handle)?;
                }
            }
        }

        Ok(CommandOutcome::Ok)
    }

    async fn status(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let summary = client.get_service(self.ctx.project_name()).await?;

        println!("{summary}");

        Ok(CommandOutcome::Ok)
    }

    async fn clean(&self) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let message = client
            .clean_project(self.ctx.project_name())
            .await
            .map_err(|err| {
                suggestions::project::project_request_failure(
                    err,
                    "Project clean failed",
                    true,
                    "cleaning your project or checking its status fail repeatedly",
                )
            })?;
        println!("{message}");

        Ok(CommandOutcome::Ok)
    }

    async fn logs(&self, args: LogsArgs) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let id = if let Some(id) = args.id {
            id
        } else {
            let proj_name = self.ctx.project_name();

            if args.latest {
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

        if args.follow {
            let mut stream = client
                .get_logs_ws(self.ctx.project_name(), &id)
                .await
                .map_err(|err| {
                    suggestions::logs::get_logs_failure(err, "Connecting to the logs stream failed")
                })?;

            while let Some(Ok(msg)) = stream.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                    match serde_json::from_str::<shuttle_common::LogItem>(&line) {
                        Ok(log) => {
                            if args.raw {
                                println!("{}", log.get_raw_line())
                            } else {
                                println!("{log}")
                            }
                        }
                        Err(err) => {
                            debug!(error = %err, "failed to parse message into log item");

                            let message = if let Ok(err) = serde_json::from_str::<ApiError>(&line) {
                                err.to_string()
                            } else {
                                "failed to parse logs, is your cargo-shuttle outdated?".to_string()
                            };

                            bail!(message);
                        }
                    }
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
                if args.raw {
                    println!("{}", log.get_raw_line())
                } else {
                    println!("{log}")
                }
            }
        }

        Ok(CommandOutcome::Ok)
    }

    async fn deployments_list(&self, page: u32, limit: u32, raw: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        if limit == 0 {
            println!();
            return Ok(CommandOutcome::Ok);
        }
        let limit = limit + 1;

        let proj_name = self.ctx.project_name();
        let mut deployments = client
            .get_deployments(proj_name, page, limit)
            .await
            .map_err(suggestions::deployment::get_deployments_list_failure)?;
        let page_hint = if deployments.len() == limit as usize {
            deployments.pop();
            true
        } else {
            false
        };
        let table = get_deployments_table(&deployments, proj_name, page, raw, page_hint);

        println!("{table}");

        if deployments.is_empty() {
            println!("Run `cargo shuttle deploy` to deploy your project.");
        } else {
            println!("Run `cargo shuttle logs <id>` to get logs for a given deployment.");
        }

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

    async fn resources_list(&self, raw: bool, show_secrets: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let resources = client
            .get_service_resources(self.ctx.project_name())
            .await
            .map_err(suggestions::resources::get_service_resources_failure)?;
        let table = get_resource_tables(&resources, self.ctx.project_name(), raw, show_secrets);

        println!("{table}");

        Ok(CommandOutcome::Ok)
    }

    async fn resource_delete(
        &self,
        resource_type: &resource::Type,
        no_confirm: bool,
    ) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();

        if !no_confirm {
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
        idx: u16,
    ) -> Result<Option<(Child, runtime::Client)>> {
        let secrets_file = run_args.secret_args.secrets.clone().or_else(|| {
            let crate_dir = service.crate_directory();
            // Prioritise crate-local prod secrets over workspace dev secrets (in the rare case that both exist)
            [
                crate_dir.join("Secrets.dev.toml"),
                crate_dir.join("Secrets.toml"),
                service.workspace_path.join("Secrets.dev.toml"),
                service.workspace_path.join("Secrets.toml"),
            ]
            .into_iter()
            .find(|f| f.exists() && f.is_file())
        });
        let secrets = if let Some(secrets_file) = secrets_file {
            trace!("Loading secrets from {}", secrets_file.display());
            if let Ok(secrets_str) = read_to_string(secrets_file) {
                let secrets = toml::from_str::<HashMap<String, String>>(&secrets_str)?;
                trace!(keys = ?secrets.keys(), "available secrets");
                secrets
            } else {
                trace!("No secrets were loaded");
                Default::default()
            }
        } else {
            trace!("No secrets file was found");
            Default::default()
        };

        trace!(path = ?service.executable_path, "using alpha runtime");
        if let Err(err) = check_version(&service.executable_path).await {
            warn!("{}", err);
            if let Some(mismatch) = err.downcast_ref::<VersionMismatchError>() {
                println!("Warning: {}.", mismatch);
                if mismatch.shuttle_runtime > mismatch.cargo_shuttle {
                    // The runtime is newer than cargo-shuttle so we
                    // should help the user to update cargo-shuttle.
                    printdoc! {"
                        Hint: A newer version of cargo-shuttle is available.
                                Check out the installation docs for how to update: {SHUTTLE_INSTALL_DOCS_URL}",
                    };
                } else {
                    printdoc! {"
                        Hint: A newer version of shuttle-runtime is available.
                        Change its version to {} in Cargo.toml to update it, or
                        run this command: cargo add shuttle-runtime@{}",
                        mismatch.cargo_shuttle,
                        mismatch.cargo_shuttle,
                    };
                }
            } else {
                return Err(err.context(
                    format!(
                        "Failed to verify the version of shuttle-runtime in {}. Is cargo targeting the correct executable?",
                        service.executable_path.display()
                    )
                ));
            }
        }
        let runtime_executable = service.executable_path.clone();

        // Child process and gRPC client for sending requests to it
        let (mut runtime, mut runtime_client) = runner::start(
            portpicker::pick_unused_port().expect("unable to find available port for gRPC server"),
            runtime_executable,
            service.workspace_path.as_path(),
        )
        .await?;

        let service_name = service.service_name()?;
        let deployment_id: Uuid = Default::default();

        let child_stdout = runtime
            .stdout
            .take()
            .context("child process did not have a handle to stdout")?;
        let mut reader = BufReader::new(child_stdout).lines();
        let service_name_clone = service_name.clone();
        let raw = run_args.raw;
        tokio::spawn(async move {
            while let Some(line) = reader.next_line().await.unwrap() {
                let log_item = LogItem::new(
                    deployment_id,
                    shuttle_common::log::Backend::Runtime(service_name_clone.clone()),
                    line,
                );

                if raw {
                    println!("{}", log_item.get_raw_line())
                } else {
                    println!("{log_item}")
                }
            }
        });

        //
        // LOADING PHASE
        //

        let load_request = tonic::Request::new(LoadRequest {
            project_name: service_name.to_string(),
            env: Environment::Local.to_string(),
            secrets: secrets.clone(),
            path: service
                .executable_path
                .clone()
                .into_os_string()
                .into_string()
                .expect("to convert path to string"),
            ..Default::default()
        });

        trace!("loading service");
        let response = runtime_client
            .load(load_request)
            .or_else(|err| async {
                runtime.kill().await?;
                Err(err)
            })
            .await?
            .into_inner();

        if !response.success {
            error!(error = response.message, "failed to load your service");
            return Ok(None);
        }

        //
        // PROVISIONING PHASE
        //

        let resources = response.resources;
        let (resources, mocked_responses) =
            Shuttle::local_provision_phase(service_name.as_str(), resources, secrets).await?;

        println!(
            "{}",
            get_resource_tables(&mocked_responses, service_name.as_str(), false, false)
        );

        //
        // START PHASE
        //

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
            resources,
        };

        trace!(?start_request, "starting service");
        let response = runtime_client
            .start(tonic::Request::new(start_request))
            .or_else(|err| async {
                runtime.kill().await?;
                Err(err)
            })
            .await?
            .into_inner();

        trace!(response = ?response,  "client response: ");
        Ok(Some((runtime, runtime_client)))
    }

    async fn local_provision_phase(
        project_name: &str,
        mut resources: Vec<Vec<u8>>,
        secrets: HashMap<String, String>,
    ) -> Result<(Vec<Vec<u8>>, Vec<resource::Response>)> {
        // for displaying the tables
        let mut mocked_responses: Vec<resource::Response> = Vec::new();
        let prov = LocalProvisioner::new()?;

        // Fail early if any bytes is invalid json
        let values = resources
            .iter()
            .map(|bytes| {
                serde_json::from_slice::<ResourceInput>(bytes)
                    .context("deserializing resource input")
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        for (bytes, shuttle_resource) in
            resources
                .iter_mut()
                .zip(values)
                // ignore non-Shuttle resource items
                .filter_map(|(bytes, value)| match value {
                    ResourceInput::Shuttle(shuttle_resource) => Some((bytes, shuttle_resource)),
                    ResourceInput::Custom(_) => None,
                })
                .map(|(bytes, shuttle_resource)| {
                    if shuttle_resource.version == RESOURCE_SCHEMA_VERSION {
                        Ok((bytes, shuttle_resource))
                    } else {
                        Err(anyhow!("
                            Shuttle resource request for {} with incompatible version found. Expected {}, found {}. \
                            Make sure that this deployer and the Shuttle resource are up to date.
                            ",
                            shuttle_resource.r#type,
                            RESOURCE_SCHEMA_VERSION,
                            shuttle_resource.version
                        ))
                    }
                }).collect::<anyhow::Result<Vec<_>>>()?.into_iter()
        {
            match shuttle_resource.r#type {
                resource::Type::Database(db_type) => {
                    let config: DbInput = serde_json::from_value(shuttle_resource.config)
                        .context("deserializing resource config")?;
                    let res = match config.local_uri {
                        Some(local_uri) => DatabaseResource::ConnectionString(local_uri),
                        None => DatabaseResource::Info(
                            prov.provision_database(Request::new(DatabaseRequest {
                                project_name: project_name.to_string(),
                                db_type: Some(db_type.into()),
                                db_name: config.db_name,
                            }))
                            .await?
                            .into_inner()
                            .into(),
                        ),
                    };
                    mocked_responses.push(resource::Response {
                        r#type: shuttle_resource.r#type,
                        config: serde_json::Value::Null,
                        data: serde_json::to_value(&res).unwrap(),
                    });
                    *bytes = serde_json::to_vec(&ShuttleResourceOutput {
                        output: res,
                        custom: shuttle_resource.custom,
                    })
                    .unwrap();
                }
                resource::Type::Secrets => {
                    // We already know the secrets at this stage, they are not provisioned like other resources
                    mocked_responses.push(resource::Response {
                        r#type: shuttle_resource.r#type,
                        config: serde_json::Value::Null,
                        data: serde_json::to_value(secrets.clone()).unwrap(),
                    });
                    *bytes = serde_json::to_vec(&ShuttleResourceOutput {
                        output: secrets.clone(),
                        custom: shuttle_resource.custom,
                    })
                    .unwrap();
                }
                resource::Type::Persist => {
                    // only show that this resource is "connected"
                    mocked_responses.push(resource::Response {
                        r#type: shuttle_resource.r#type,
                        config: serde_json::Value::Null,
                        data: serde_json::Value::Null,
                    });
                }
                resource::Type::Container => {
                    let config = serde_json::from_value(shuttle_resource.config)
                        .context("deserializing resource config")?;
                    let res = prov.start_container(config).await?;
                    *bytes = serde_json::to_vec(&ShuttleResourceOutput {
                        output: res,
                        custom: shuttle_resource.custom,
                    })
                    .unwrap();
                }
            }
        }

        Ok((resources, mocked_responses))
    }

    async fn stop_runtime(
        runtime: &mut Child,
        runtime_client: &mut runtime::Client,
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
        trace!(response = ?response, "client stop response: ");
        Ok(())
    }

    async fn add_runtime_info(
        runtime: Option<(Child, runtime::Client)>,
        existing_runtimes: &mut Vec<(Child, runtime::Client)>,
    ) -> Result<(), Status> {
        match runtime {
            Some(inner) => {
                trace!("Adding runtime PID: {:?}", inner.0.id());
                existing_runtimes.push(inner);
            }
            None => {
                trace!("Runtime error: No runtime process. Crashed during startup?");
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

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);
        tokio::task::spawn(async move {
            while let Some(line) = rx.recv().await {
                println!("{line}");
            }
        });

        let working_directory = self.ctx.working_directory();

        trace!("building project");
        println!(
            "{} {}",
            "    Building".bold().green(),
            working_directory.display()
        );

        build_workspace(working_directory, run_args.release, tx, false).await
    }

    #[cfg(target_family = "unix")]
    async fn local_run(&self, mut run_args: RunArgs) -> Result<CommandOutcome> {
        debug!("starting local run");
        let services = self.pre_local_run(&run_args).await?;

        let mut sigterm_notif =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Can not get the SIGTERM signal receptor");
        let mut sigint_notif =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("Can not get the SIGINT signal receptor");

        // Start all the services.
        let mut runtimes: Vec<(Child, runtime::Client)> = Vec::new();

        Shuttle::find_available_port(&mut run_args, services.len());

        let mut signal_received = false;
        for (i, service) in services.iter().enumerate() {
            // We must cover the case of starting multiple workspace services and receiving a signal in parallel.
            // This must stop all the existing runtimes and creating new ones.
            signal_received = tokio::select! {
                res = Shuttle::spin_local_runtime(&run_args, service, i as u16) => {
                    match res {
                        Ok(runtime) => {
                            Shuttle::add_runtime_info(runtime, &mut runtimes).await?;
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
                    Shuttle::stop_runtime(&mut rt, &mut rt_client).await.unwrap_or_else(|err| {
                        trace!(status = ?err, "stopping the runtime errored out");
                    });
                    true
                },
                _ = sigint_notif.recv() => {
                    println!(
                        "cargo-shuttle received SIGINT. Killing all the runtimes..."
                    );
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

        // Start all the services.
        let mut runtimes: Vec<(Child, runtime::Client)> = Vec::new();

        Shuttle::find_available_port(&mut run_args, services.len());

        let mut signal_received = false;
        for (i, service) in services.iter().enumerate() {
            signal_received = tokio::select! {
                res = Shuttle::spin_local_runtime(&run_args, service, i as u16) => {
                    Shuttle::add_runtime_info(res.unwrap(), &mut runtimes).await?;
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

            let dirty = is_dirty(&repo);
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

        deployment_req.data = self.make_archive(args.secret_args.secrets.clone())?;
        if deployment_req.data.len() > CREATE_SERVICE_BODY_LIMIT {
            bail!(
                r#"The project is too large - the limit is {} MB. \
                Your project archive is {:.1} MB. \
                Run with `cargo shuttle --debug` to see which files are being packed."#,
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
            if let Some(Ok(msg)) = stream.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                    let log_item = match serde_json::from_str::<shuttle_common::LogItem>(&line) {
                        Ok(log_item) => log_item,
                        Err(err) => {
                            debug!(error = %err, "failed to parse message into log item");

                            let message = if let Ok(err) = serde_json::from_str::<ApiError>(&line) {
                                err.to_string()
                            } else {
                                "failed to parse logs, is your cargo-shuttle outdated?".to_string()
                            };

                            bail!(message);
                        }
                    };

                    if args.raw {
                        println!("{}", log_item.get_raw_line())
                    } else {
                        println!("{log_item}")
                    }

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
                sleep(Duration::from_millis(100)).await;
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
        sleep(Duration::from_millis(500)).await;

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
        let resources = get_resource_tables(&resources, self.ctx.project_name(), false, false);

        println!("{resources}{service}");

        Ok(CommandOutcome::Ok)
    }

    async fn project_start(&self, idle_minutes: u64) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        let config = &project::Config { idle_minutes };

        let p = self.ctx.project_name();
        wait_with_spinner(|i, pb| async move {
            let project = if i == 0 {
                client.create_project(p, config).await?
            } else {
                client.get_project(p).await?
            };
            pb.set_message(format!("{project}"));

            let done = [
                project::State::Ready,
                project::State::Errored {
                    message: Default::default(),
                },
            ]
            .contains(&project.state);

            if done {
                Ok(Some(move || {
                    println!("{project}");
                }))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|err| {
            suggestions::project::project_request_failure(
                err,
                "Project creation failed",
                true,
                "the project creation or retrieving the status fails repeatedly",
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

    async fn project_restart(&self, idle_minutes: u64) -> Result<CommandOutcome> {
        self.project_stop()
            .await
            .map_err(suggestions::project::project_restart_failure)?;
        self.project_start(idle_minutes)
            .await
            .map_err(suggestions::project::project_restart_failure)?;

        Ok(CommandOutcome::Ok)
    }

    async fn projects_list(&self, page: u32, limit: u32, raw: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        if limit == 0 {
            println!();
            return Ok(CommandOutcome::Ok);
        }
        let limit = limit + 1;

        let mut projects = client.get_projects_list(page, limit).await.map_err(|err| {
            suggestions::project::project_request_failure(
                err,
                "Getting projects list failed",
                false,
                "getting the projects list fails repeatedly",
            )
        })?;
        let page_hint = if projects.len() == limit as usize {
            projects.pop();
            true
        } else {
            false
        };
        let projects_table = project::get_projects_table(&projects, page, raw, page_hint);

        println!("{projects_table}");

        Ok(CommandOutcome::Ok)
    }

    async fn project_status(&self, follow: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();
        if follow {
            let p = self.ctx.project_name();
            wait_with_spinner(|_, pb| async move {
                let project = client.get_project(p).await?;
                pb.set_message(format!("{project}"));

                let done = [
                    project::State::Ready,
                    project::State::Destroyed,
                    project::State::Errored {
                        message: Default::default(),
                    },
                ]
                .contains(&project.state);

                if done {
                    Ok(Some(move || {
                        println!("{project}");
                    }))
                } else {
                    Ok(None)
                }
            })
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
                        "getting project status failed repeatedly",
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

        let p = self.ctx.project_name();
        wait_with_spinner(|i, pb| async move {
            let project = if i == 0 {
                client.stop_project(p).await?
            } else {
                client.get_project(p).await?
            };
            pb.set_message(format!("{project}"));

            let done = [
                project::State::Destroyed,
                project::State::Errored {
                    message: Default::default(),
                },
            ]
            .contains(&project.state);

            if done {
                Ok(Some(move || {
                    println!("{project}");
                }))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|err| {
            suggestions::project::project_request_failure(
                err,
                "Project stop failed",
                true,
                "stopping the project or getting project status fails repeatedly",
            )
        })?;
        println!("Run `cargo shuttle project start` to recreate project environment on Shuttle.");

        Ok(CommandOutcome::Ok)
    }

    async fn project_delete(&self, no_confirm: bool) -> Result<CommandOutcome> {
        let client = self.client.as_ref().unwrap();

        if !no_confirm {
            println!(
                "{}",
                formatdoc!(
                    r#"
                    WARNING:
                        Are you sure you want to delete "{}"?
                        This will...
                        - Delete any databases, secrets, and shuttle-persist data in this project.
                        - Delete any custom domains linked to this project.
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
        }

        client
            .delete_project(self.ctx.project_name())
            .await
            .map_err(|err| {
                suggestions::project::project_request_failure(
                    err,
                    "Project delete failed",
                    true,
                    "deleting the project or getting project status fails repeatedly",
                )
            })?;

        println!("Deleted project");

        Ok(CommandOutcome::Ok)
    }

    fn make_archive(&self, secrets_file: Option<PathBuf>) -> Result<Vec<u8>> {
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

        if let Some(secrets_file) = secrets_file.clone() {
            entries.push(secrets_file);
        } else {
            // Default: Include all Secrets.toml files
            globs.add(Glob::new("**/Secrets.toml").unwrap());
        }

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

            let mut name = path
                .strip_prefix(working_directory.parent().context("get parent dir")?)
                .context("strip prefix of path")?
                .to_owned();

            // if this is the custom secrets file, rename it to Secrets.toml
            if secrets_file.as_ref().is_some_and(|sf| sf == &path) {
                name.pop();
                name.push("Secrets.toml");
            }

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
}

// /// Can be used during testing
// async fn get_templates_schema() -> Result<TemplatesSchema> {
//     Ok(toml::from_str(include_str!(
//         "../../examples/templates.toml"
//     ))?)
// }
async fn get_templates_schema() -> Result<TemplatesSchema> {
    let client = reqwest::Client::new();
    Ok(toml::from_str(
        &client
            .get(shuttle_common::constants::EXAMPLES_TEMPLATES_TOML)
            .send()
            .await?
            .text()
            .await?,
    )?)
}

fn is_dirty(repo: &Repository) -> Result<()> {
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
        writeln!(error, "To proceed despite this and include the uncommitted changes, pass the `--allow-dirty` or `--ad` flag").expect("to append error");

        bail!(error);
    }

    Ok(())
}

async fn check_version(runtime_path: &Path) -> Result<()> {
    debug!(
        "Checking version of runtime binary at {}",
        runtime_path.display()
    );

    // should always be a valid semver
    let my_version = semver::Version::from_str(VERSION).unwrap();

    if !runtime_path.try_exists()? {
        bail!("shuttle-runtime binary not found");
    }

    // Get runtime version from shuttle-runtime cli
    // It should print the version and exit immediately, so a timeout is used to not launch servers with non-Shuttle setups
    let stdout = tokio::time::timeout(Duration::from_millis(3000), async move {
        tokio::process::Command::new(runtime_path)
            .arg("--version")
            .kill_on_drop(true) // if the binary does not halt on its own, not killing it will leak child processes
            .output()
            .await
            .context("Failed to run the shuttle-runtime binary to check its version")
            .map(|o| o.stdout)
    })
    .await
    .context("Checking the version of shuttle-runtime timed out. Make sure the executable is using #[shuttle-runtime::main].")??;

    // Parse the version, splitting the version from the name and
    // and pass it to `to_semver()`.
    let runtime_version = semver::Version::from_str(
        std::str::from_utf8(&stdout)
            .context("shuttle-runtime version should be valid utf8")?
            .split_once(' ')
            .context("shuttle-runtime version should be in the `name version` format")?
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

/// Calls async function `f` in a loop with sleep,
/// while providing iteration count and reference to update the progress bar.
/// `f` returns Some with a cleanup function if done.
/// The cleanup function is called after teardown of progress bar,
/// and its return value is returned from here.
async fn wait_with_spinner<Fut, C, O>(
    f: impl Fn(usize, ProgressBar) -> Fut,
) -> Result<O, anyhow::Error>
where
    Fut: std::future::Future<Output = Result<Option<C>>>,
    C: FnOnce() -> O,
{
    let progress_bar = create_spinner();
    let mut count = 0usize;
    let cleanup = loop {
        if let Some(cleanup) = f(count, progress_bar.clone()).await? {
            break cleanup;
        }
        count += 1;
        sleep(Duration::from_millis(300)).await;
    };
    progress_bar.finish_and_clear();

    Ok(cleanup())
}

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
    use tar::Archive;

    use crate::args::{DeployArgs, ProjectArgs, SecretsArgs};
    use crate::Shuttle;
    use std::fs::{self, canonicalize};
    use std::path::PathBuf;

    pub fn path_from_workspace_root(path: &str) -> PathBuf {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path);

        dunce::canonicalize(path).unwrap()
    }

    fn get_archive_entries(project_args: ProjectArgs, deploy_args: DeployArgs) -> Vec<String> {
        let mut shuttle = Shuttle::new().unwrap();
        shuttle.load_project(&project_args).unwrap();

        let archive = shuttle
            .make_archive(deploy_args.secret_args.secrets)
            .unwrap();

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
            working_directory: working_directory.clone(),
            name: Some("archiving-test".to_owned()),
        };
        let mut entries = get_archive_entries(project_args.clone(), Default::default());
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

        fs::remove_file(working_directory.join("Secrets.toml")).unwrap();
        let mut entries = get_archive_entries(
            project_args,
            DeployArgs {
                secret_args: SecretsArgs {
                    secrets: Some(working_directory.join("Secrets.toml.example")),
                },
                ..Default::default()
            },
        );
        entries.sort();

        assert_eq!(
            entries,
            vec![
                ".gitignore",
                ".ignore",
                "Cargo.toml",
                "Secrets.toml", // got moved here
                // Secrets.toml.example was the given secrets file, so it got moved
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
    fn finds_workspace_root() {
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
