mod args;
pub mod builder;
pub mod config;
mod init;
mod provisioner_server;
mod util;

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use args::DeploymentTrackingArgs;
use chrono::Utc;
use clap::{parser::ValueSource, CommandFactory, FromArgMatches};
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use futures::{SinkExt, StreamExt};
use git2::Repository;
use globset::{Glob, GlobSetBuilder};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use indicatif::ProgressBar;
use indoc::formatdoc;
use reqwest::header::HeaderMap;
use shuttle_api_client::ShuttleApiClient;
use shuttle_builder::render_rust_dockerfile;
use shuttle_common::{
    constants::{
        headers::X_CARGO_SHUTTLE_VERSION, other_env_api_url, EXAMPLES_REPO, SHUTTLE_API_URL,
        SHUTTLE_CONSOLE_URL, TEMPLATES_SCHEMA_VERSION,
    },
    models::{
        auth::{KeyMessage, TokenMessage},
        deployment::{
            BuildArgs as CommonBuildArgs, BuildMeta, DeploymentRequest,
            DeploymentRequestBuildArchive, DeploymentRequestImage, DeploymentResponse,
            DeploymentState, Environment, GIT_STRINGS_MAX_LENGTH,
        },
        error::ApiError,
        log::LogItem,
        project::ProjectUpdateRequest,
        resource::ResourceType,
    },
    tables::{deployments_table, get_certificates_table, get_projects_table, get_resource_tables},
};
use shuttle_ifc::parse_infra_from_code;
use strum::{EnumMessage, VariantArray};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};
use util::cargo_green_eprintln;
use zip::write::FileOptions;

use crate::args::{
    BuildArgs, CertificateCommand, ConfirmationArgs, DeployArgs, DeploymentCommand,
    GenerateCommand, InitArgs, LoginArgs, LogoutArgs, LogsArgs, McpCommand, OutputMode,
    ProjectCommand, ProjectUpdateCommand, ResourceCommand, SecretsArgs, TableArgs,
    TemplateLocation,
};
pub use crate::args::{BuildArgsShared, Command, ProjectArgs, RunArgs, ShuttleArgs};
use crate::builder::{
    cargo_build, find_first_shuttle_package, gather_rust_build_args, BuiltService,
};
use crate::config::RequestContext;
use crate::provisioner_server::{ProvApiState, ProvisionerServer};
use crate::util::{
    bacon, cargo_metadata, check_and_warn_runtime_version, generate_completions, generate_manpage,
    get_templates_schema, is_dirty, open_gh_issue, read_ws_until_text, update_cargo_shuttle,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the args and whether the PATH arg of the init command was explicitly given
pub fn parse_args() -> (ShuttleArgs, bool) {
    let matches = ShuttleArgs::command().get_matches();
    let mut args =
        ShuttleArgs::from_arg_matches(&matches).expect("args to already be parsed successfully");
    let provided_path_to_init = matches
        .subcommand_matches("init")
        .is_some_and(|init_matches| {
            init_matches.value_source("path") == Some(ValueSource::CommandLine)
        });

    // don't use an override if production is targetted
    if args
        .api_env
        .as_ref()
        .is_some_and(|e| e == "prod" || e == "production")
    {
        args.api_env = None;
    }

    (args, provided_path_to_init)
}

pub fn setup_tracing(debug: bool) {
    registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(
            // let user set RUST_LOG if they want to
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                if debug {
                    EnvFilter::new("info,cargo_shuttle=trace,shuttle=trace")
                } else {
                    EnvFilter::default()
                }
            }),
        )
        .init();
}

#[derive(PartialEq)]
pub enum Binary {
    CargoShuttle,
    Shuttle,
}

impl Binary {
    pub fn name(&self) -> String {
        match self {
            Self::CargoShuttle => "cargo-shuttle".to_owned(),
            Self::Shuttle => "shuttle".to_owned(),
        }
    }
}

pub struct Shuttle {
    ctx: RequestContext,
    client: Option<ShuttleApiClient>,
    output_mode: OutputMode,
    /// Alter behaviour based on which CLI is used
    bin: Binary,
}

impl Shuttle {
    pub fn new(bin: Binary, env_override: Option<String>) -> Result<Self> {
        let ctx = RequestContext::load_global(env_override.inspect(|e| {
            eprintln!(
                "{}",
                format!("INFO: Using non-default global config file: {e}").yellow(),
            )
        }))?;
        Ok(Self {
            ctx,
            client: None,
            output_mode: OutputMode::Normal,
            bin,
        })
    }

    pub async fn run(mut self, args: ShuttleArgs, provided_path_to_init: bool) -> Result<()> {
        self.output_mode = args.output_mode;

        // Set up the API client for all commands that call the API
        if matches!(
            args.cmd,
            Command::Init(..)
                | Command::Deploy(..)
                | Command::Logs { .. }
                | Command::Account
                | Command::Login(..)
                | Command::Logout(..)
                | Command::Deployment(..)
                | Command::Resource(..)
                | Command::Certificate(..)
                | Command::Project(..)
        ) {
            let api_url = args
                .api_url
                // calculate env-specific url if no explicit url given but an env was given
                .or_else(|| args.api_env.as_ref().map(|env| other_env_api_url(env)))
                // add /admin prefix if in admin mode
                .map(|u| if args.admin { format!("{u}/admin") } else { u });
            if let Some(ref url) = api_url {
                if url != SHUTTLE_API_URL {
                    eprintln!(
                        "{}",
                        format!("INFO: Targeting non-default API: {url}").yellow(),
                    );
                }
                if url.ends_with('/') {
                    eprintln!("WARNING: API URL is probably incorrect. Ends with '/': {url}");
                }
            }
            self.ctx.set_api_url(api_url);

            let client = ShuttleApiClient::new(
                self.ctx.api_url(),
                self.ctx.api_key().ok(),
                Some(
                    HeaderMap::try_from(&HashMap::from([(
                        X_CARGO_SHUTTLE_VERSION.clone(),
                        crate::VERSION.to_owned(),
                    )]))
                    .unwrap(),
                ),
                None,
            );
            self.client = Some(client);
        }

        // Load project context for all commands that need to know which project is being targetted
        if matches!(
            args.cmd,
            Command::Deploy(..)
                | Command::Deployment(..)
                | Command::Resource(..)
                | Command::Certificate(..)
                | Command::Project(
                    // ProjectCommand::List does not need to know which project we are in
                    // ProjectCommand::Create is handled separately and will always make the POST call
                    ProjectCommand::Update(..)
                        | ProjectCommand::Status
                        | ProjectCommand::Delete { .. }
                        | ProjectCommand::Link
                )
                | Command::Logs { .. }
        ) {
            // Command::Run and Command::Build use `load_local_config` (below) instead of `load_project_id` since they don't target a project in the API
            self.load_project_id(
                &args.project_args,
                matches!(args.cmd, Command::Project(ProjectCommand::Link)),
                // Only 'deploy' should create a project if the provided name is not found in the project list
                matches!(args.cmd, Command::Deploy(..)),
            )
            .await?;
        }

        match args.cmd {
            Command::Init(init_args) => {
                self.init(
                    init_args,
                    args.project_args,
                    provided_path_to_init,
                    args.offline,
                )
                .await
            }
            Command::Generate(cmd) => match cmd {
                GenerateCommand::Manpage => generate_manpage(),
                GenerateCommand::Shell { shell, output_file } => {
                    generate_completions(self.bin, shell, output_file)
                }
            },
            Command::Account => self.account().await,
            Command::Login(login_args) => self.login(login_args, args.offline, true).await,
            Command::Logout(logout_args) => self.logout(logout_args).await,
            Command::Feedback => open_gh_issue(),
            Command::Run(run_args) => {
                self.ctx.load_local_config(&args.project_args)?;
                self.local_run(run_args, args.debug).await
            }
            Command::Build(build_args) => {
                self.ctx.load_local_config(&args.project_args)?;
                self.build(&build_args).await
            }
            Command::Deploy(deploy_args) => self.deploy(deploy_args).await,
            Command::Logs(logs_args) => self.logs(logs_args).await,
            Command::Deployment(cmd) => match cmd {
                DeploymentCommand::List { page, limit, table } => {
                    self.deployments_list(page, limit, table).await
                }
                DeploymentCommand::Status { deployment_id } => {
                    self.deployment_get(deployment_id).await
                }
                DeploymentCommand::Redeploy {
                    deployment_id,
                    tracking_args,
                } => self.deployment_redeploy(deployment_id, tracking_args).await,
                DeploymentCommand::Stop { tracking_args } => {
                    self.deployment_stop(tracking_args).await
                }
            },
            Command::Resource(cmd) => match cmd {
                ResourceCommand::List {
                    table,
                    show_secrets,
                } => self.resources_list(table, show_secrets).await,
                ResourceCommand::Delete {
                    resource_type,
                    confirmation: ConfirmationArgs { yes },
                } => self.resource_delete(&resource_type, yes).await,
                ResourceCommand::Dump { resource_type } => self.resource_dump(&resource_type).await,
            },
            Command::Certificate(cmd) => match cmd {
                CertificateCommand::Add { domain } => self.add_certificate(domain).await,
                CertificateCommand::List { table } => self.list_certificates(table).await,
                CertificateCommand::Delete {
                    domain,
                    confirmation: ConfirmationArgs { yes },
                } => self.delete_certificate(domain, yes).await,
            },
            Command::Project(cmd) => match cmd {
                ProjectCommand::Create => self.project_create(args.project_args.name).await,
                ProjectCommand::Update(cmd) => match cmd {
                    ProjectUpdateCommand::Name { new_name } => self.project_rename(new_name).await,
                },
                ProjectCommand::Status => self.project_status().await,
                ProjectCommand::List { table, .. } => self.projects_list(table).await,
                ProjectCommand::Delete(ConfirmationArgs { yes }) => self.project_delete(yes).await,
                ProjectCommand::Link => Ok(()), // logic is done in `load_project_id` in previous step
            },
            Command::Upgrade { preview } => update_cargo_shuttle(preview).await,
            Command::Mcp(cmd) => match cmd {
                McpCommand::Start => shuttle_mcp::run_mcp_server().await,
            },
        }
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
    ) -> Result<()> {
        // Turns the template or git args (if present) to a repo+folder.
        let git_template = args.git_template()?;
        let no_git = args.no_git;

        let needs_name = project_args.name.is_none();
        let needs_template = git_template.is_none();
        let needs_path = !provided_path_to_init;
        let needs_login = self.ctx.api_key().is_err() && args.login_args.api_key.is_none();
        let should_link = project_args.id.is_some();
        let interactive = needs_name || needs_template || needs_path || needs_login;

        let theme = ColorfulTheme::default();

        // 1. Log in (if not logged in yet)
        if needs_login {
            eprintln!("First, let's log in to your Shuttle account.");
            self.login(args.login_args.clone(), offline, false).await?;
            eprintln!();
        } else if args.login_args.api_key.is_some() {
            self.login(args.login_args.clone(), offline, false).await?;
        }

        // 2. Ask for project name or validate the given one
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
                eprintln!(r#"Type the same name again to use "{}" anyways."#, name);
                prev_name = Some(name);
            } else {
                // don't continue if non-interactive
                bail!(
                    "Invalid or unavailable project name. Use `--force-name` to use this project name anyways."
                );
            }
        }
        if needs_name {
            eprintln!();
        }

        // 3. Confirm the project directory
        let path = if needs_path {
            let path = args
                .path
                .join(project_args.name.as_ref().expect("name should be set"));

            loop {
                eprintln!("Where should we create this project?");

                let directory_str: String = Input::with_theme(&theme)
                    .with_prompt("Directory")
                    .default(format!("{}", path.display()))
                    .interact()?;
                eprintln!();

                let path = args::create_and_parse_path(OsString::from(directory_str))?;

                if fs::read_dir(&path)
                    .expect("init dir to exist and list entries")
                    .count()
                    > 0
                    && !Confirm::with_theme(&theme)
                        .with_prompt("Target directory is not empty. Are you sure?")
                        .default(true)
                        .interact()?
                {
                    eprintln!();
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
                            eprintln!(
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
                            eprintln!(
                                "{}",
                                "Template list with incompatible version found. Consider upgrading Shuttle CLI. Falling back to internal list."
                                    .yellow()
                            );

                            None
                        })
                };
                if let Some(schema) = schema {
                    eprintln!("What type of project template would you like to start from?");
                    let i = Select::with_theme(&theme)
                        .items(&[
                            "A Hello World app in a supported framework",
                            "Browse our full library of templates", // TODO(when templates page is live): Add link to it?
                        ])
                        .clear(false)
                        .default(0)
                        .interact()?;
                    eprintln!();
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
                        eprintln!();
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
                        eprintln!();
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
                    eprintln!("Shuttle works with many frameworks. Which one do you want to use?");
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
                    eprintln!();
                    frameworks[index].template()
                }
            }
        };

        // 5. Initialize locally
        crate::init::generate_project(
            path.clone(),
            project_args
                .name
                .as_ref()
                .expect("to have a project name provided"),
            &template,
            no_git,
        )?;
        eprintln!();

        // 6. Confirm that the user wants to create the project on Shuttle
        let should_create_project = if should_link {
            // user wants to link project that already exists
            false
        } else if !interactive {
            // non-interactive mode: use value of arg
            args.create_project
        } else if args.create_project {
            // interactive and arg is true
            true
        } else {
            // interactive and arg was not set, so ask
            let name = project_args
                .name
                .as_ref()
                .expect("to have a project name provided");

            let should_create = Confirm::with_theme(&theme)
                .with_prompt(format!(
                    r#"Create a project on Shuttle with the name "{name}"?"#
                ))
                .default(true)
                .interact()?;
            eprintln!();
            should_create
        };

        if should_link || should_create_project {
            // Set the project working directory path to the init path,
            // so `load_project_id` is ran with the correct project path when linking
            project_args.working_directory.clone_from(&path);

            self.load_project_id(&project_args, true, true).await?;
        }

        if std::env::current_dir().is_ok_and(|d| d != path) {
            eprintln!("You can `cd` to the directory, then:");
        }
        eprintln!("Run `shuttle deploy` to deploy it to Shuttle.");

        Ok(())
    }

    /// Return value: true -> success or unknown. false -> try again.
    async fn check_project_name(&self, project_args: &mut ProjectArgs, name: String) -> bool {
        let client = self.client.as_ref().unwrap();
        match client
            .check_project_name(&name)
            .await
            .map(|r| r.into_inner())
        {
            Ok(true) => {
                project_args.name = Some(name);

                true
            }
            Ok(false) => {
                // should not be possible
                panic!("Unexpected API response");
            }
            Err(e) => {
                // If API error contains message regarding format of error name, print that error and prompt again
                if let Ok(api_error) = e.downcast::<ApiError>() {
                    // If the returned error string changes, this could break
                    if api_error.message().contains("Invalid project name") {
                        eprintln!("{}", api_error.message().yellow());
                        eprintln!("{}", "Try a different name.".yellow());
                        return false;
                    }
                }
                // Else, the API error was about something else.
                // Ignore and keep going to not prevent the flow of the init command.
                project_args.name = Some(name);
                eprintln!(
                    "{}",
                    "Failed to check if project name is available.".yellow()
                );

                true
            }
        }
    }

    /// Ensures a project id is known, either by explicit --id/--name args or config file(s)
    /// or by asking user to link the project folder.
    pub async fn load_project_id(
        &mut self,
        project_args: &ProjectArgs,
        do_linking: bool,
        create_missing_project: bool,
    ) -> Result<()> {
        trace!("project arguments: {project_args:?}");

        self.ctx.load_local_config(project_args)?;
        // load project id from args if given or from internal config file if present
        self.ctx.load_local_internal_config(project_args)?;

        // If project id was not given via arg but a name was, try to translate the project name to a project id.
        // (A --name from args takes precedence over an id from internal config.)
        if project_args.id.is_none() {
            if let Some(name) = project_args.name.as_ref() {
                let client = self.client.as_ref().unwrap();
                trace!(%name, "looking up project id from project name");
                if let Some(proj) = client
                    .get_projects_list()
                    .await?
                    .into_inner()
                    .projects
                    .into_iter()
                    .find(|p| p.name == *name)
                {
                    trace!("found project by name");
                    self.ctx.set_project_id(proj.id);
                } else {
                    trace!("did not find project by name");
                    if create_missing_project {
                        trace!("creating project since it was not found");
                        // This is a side effect (non-primary output), so OutputMode::Json is not considered
                        let proj = client.create_project(name).await?.into_inner();
                        eprintln!("Created project '{}' with id {}", proj.name, proj.id);
                        self.ctx.set_project_id(proj.id);
                    } else if do_linking {
                        self.project_link_interactive().await?;
                        return Ok(());
                    } else {
                        bail!(
                            "Project with name '{}' not found in your project list. \
                            Use 'shuttle project link' to create it or link an existing project.",
                            name
                        );
                    }
                }
            }
        }

        match (self.ctx.project_id_found(), do_linking) {
            (true, true) => {
                let arg_given = project_args.id.is_some() || project_args.name.is_some();
                if arg_given {
                    // project id was found via explicitly given arg, save config
                    eprintln!("Linking to project {}", self.ctx.project_id());
                    self.ctx.save_local_internal()?;
                } else {
                    // project id was found but not given via arg, ask the user interactively
                    self.project_link_interactive().await?;
                }
            }
            // if project id is known, we are done and nothing more to do
            (true, false) => (),
            // we still don't know the project id, so ask the user interactively
            (false, _) => {
                trace!("no project id found");
                self.project_link_interactive().await?;
            }
        }

        Ok(())
    }

    async fn project_link_interactive(&mut self) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let projs = client.get_projects_list().await?.into_inner().projects;

        let theme = ColorfulTheme::default();

        let selected_project = if projs.is_empty() {
            eprintln!("Create a new project to link to this directory:");

            None
        } else {
            eprintln!("Which project do you want to link this directory to?");

            let mut items = projs
                .iter()
                .map(|p| {
                    if let Some(team_id) = p.team_id.as_ref() {
                        format!("Team {}: {}", team_id, p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect::<Vec<_>>();
            items.extend_from_slice(&["[CREATE NEW]".to_string()]);
            let index = Select::with_theme(&theme)
                .items(&items)
                .default(0)
                .interact()?;

            if index == projs.len() {
                // last item selected (create new)
                None
            } else {
                Some(projs[index].clone())
            }
        };

        let proj = match selected_project {
            Some(proj) => proj,
            None => {
                let name: String = Input::with_theme(&theme)
                    .with_prompt("Project name")
                    .interact()?;

                // This is a side effect (non-primary output), so OutputMode::Json is not considered
                let proj = client.create_project(&name).await?.into_inner();
                eprintln!("Created project '{}' with id {}", proj.name, proj.id);

                proj
            }
        };

        eprintln!("Linking to project '{}' with id {}", proj.name, proj.id);
        self.ctx.set_project_id(proj.id);
        self.ctx.save_local_internal()?;

        Ok(())
    }

    async fn account(&self) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let r = client.get_current_user().await?;
        match self.output_mode {
            OutputMode::Normal => {
                print!("{}", r.into_inner().to_string_colored());
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    /// Log in with the given API key or after prompting the user for one.
    async fn login(&mut self, login_args: LoginArgs, offline: bool, login_cmd: bool) -> Result<()> {
        let api_key = match login_args.api_key {
            Some(api_key) => api_key,
            None => {
                if login_args.prompt {
                    Password::with_theme(&ColorfulTheme::default())
                        .with_prompt("API key")
                        .validate_with(|input: &String| {
                            if input.is_empty() {
                                return Err("Empty API key was provided");
                            }
                            Ok(())
                        })
                        .interact()?
                } else {
                    // device auth flow via Shuttle Console
                    self.device_auth(login_args.console_url).await?
                }
            }
        };

        self.ctx.set_api_key(api_key.clone())?;

        if let Some(client) = self.client.as_mut() {
            client.api_key = Some(api_key);

            if offline {
                eprintln!("INFO: Skipping API key verification");
            } else {
                let (user, raw_json) = client
                    .get_current_user()
                    .await
                    .context("failed to check API key validity")?
                    .into_parts();
                if login_cmd {
                    match self.output_mode {
                        OutputMode::Normal => {
                            println!("Logged in as {}", user.id.bold());
                        }
                        OutputMode::Json => {
                            println!("{}", raw_json);
                        }
                    }
                } else {
                    eprintln!("Logged in as {}", user.id.bold());
                }
            }
        }

        Ok(())
    }

    async fn device_auth(&self, console_url: Option<String>) -> Result<String> {
        let client = self.client.as_ref().unwrap();

        // should not have trailing slash
        if let Some(u) = console_url.as_ref() {
            if u.ends_with('/') {
                eprintln!("WARNING: Console URL is probably incorrect. Ends with '/': {u}");
            }
        }

        let (mut tx, mut rx) = client.get_device_auth_ws().await?.split();

        // keep the socket alive with ping/pong
        let pinger = tokio::spawn(async move {
            loop {
                if let Err(e) = tx.send(Message::Ping(Default::default())).await {
                    error!(error = %e, "Error when pinging websocket");
                    break;
                };
                sleep(Duration::from_secs(20)).await;
            }
        });

        let token_message = read_ws_until_text(&mut rx).await?;
        let Some(token_message) = token_message else {
            bail!("Did not receive device auth token over websocket");
        };
        let token_message = serde_json::from_str::<TokenMessage>(&token_message)?;
        let token = token_message.token;

        // use provided url if it exists, otherwise fall back to old behavior of constructing it here
        let url = token_message.url.unwrap_or_else(|| {
            format!(
                "{}/device-auth?token={}",
                console_url.as_deref().unwrap_or(SHUTTLE_CONSOLE_URL),
                token
            )
        });
        let _ = webbrowser::open(&url);
        eprintln!("Complete login in Shuttle Console to authenticate the Shuttle CLI.");
        eprintln!("If your browser did not automatically open, go to {url}");
        eprintln!();
        eprintln!("{}", format!("Token: {token}").bold());
        eprintln!();

        let key = read_ws_until_text(&mut rx).await?;
        let Some(key) = key else {
            bail!("Failed to receive API key over websocket");
        };
        let key = serde_json::from_str::<KeyMessage>(&key)?.api_key;

        pinger.abort();

        Ok(key)
    }

    async fn logout(&mut self, logout_args: LogoutArgs) -> Result<()> {
        if logout_args.reset_api_key {
            let client = self.client.as_ref().unwrap();
            client.reset_api_key().await.context("Resetting API key")?;
            eprintln!("Successfully reset the API key.");
        }
        self.ctx.clear_api_key()?;
        eprintln!("Successfully logged out.");
        eprintln!(" -> Use `shuttle login` to log in again.");

        Ok(())
    }

    async fn deployment_stop(&self, tracking_args: DeploymentTrackingArgs) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let pid = self.ctx.project_id();
        let res = client.stop_service(pid).await?.into_inner();
        println!("{res}");

        if tracking_args.no_follow {
            return Ok(());
        }

        wait_with_spinner(2000, |_, pb| async move {
            let (deployment, raw_json) = client.get_current_deployment(pid).await?.into_parts();

            let get_cleanup = |d: Option<DeploymentResponse>| {
                move || {
                    if let Some(d) = d {
                        match self.output_mode {
                            OutputMode::Normal => {
                                eprintln!("{}", d.to_string_colored());
                            }
                            OutputMode::Json => {
                                // last deployment response already printed
                            }
                        }
                    }
                }
            };
            let Some(deployment) = deployment else {
                return Ok(Some(get_cleanup(None)));
            };

            let state = deployment.state.clone();
            match self.output_mode {
                OutputMode::Normal => {
                    pb.set_message(deployment.to_string_summary_colored());
                }
                OutputMode::Json => {
                    println!("{}", raw_json);
                }
            }
            let cleanup = get_cleanup(Some(deployment));
            match state {
                DeploymentState::Pending
                | DeploymentState::Stopping
                | DeploymentState::InProgress
                | DeploymentState::Running => Ok(None),
                DeploymentState::Building // a building deployment should take it back to InProgress then Running, so don't follow that sequence
                | DeploymentState::Failed
                | DeploymentState::Stopped
                | DeploymentState::Unknown(_) => Ok(Some(cleanup)),
            }
        })
        .await?;

        Ok(())
    }

    async fn logs(&self, args: LogsArgs) -> Result<()> {
        if args.follow {
            eprintln!("Streamed logs are not yet supported on the shuttle.dev platform.");
            return Ok(());
        }
        if args.tail.is_some() | args.head.is_some() {
            eprintln!("Fetching log ranges are not yet supported on the shuttle.dev platform.");
            return Ok(());
        }
        let client = self.client.as_ref().unwrap();
        let pid = self.ctx.project_id();
        let r = if args.all_deployments {
            client.get_project_logs(pid).await?
        } else {
            let id = if args.latest {
                // Find latest deployment (not always an active one)
                let deployments = client
                    .get_deployments(pid, 1, 1)
                    .await?
                    .into_inner()
                    .deployments;
                let Some(most_recent) = deployments.into_iter().next() else {
                    println!("No deployments found");
                    return Ok(());
                };
                eprintln!("Getting logs from: {}", most_recent.id);
                most_recent.id
            } else if let Some(id) = args.deployment_id {
                id
            } else {
                let Some(current) = client.get_current_deployment(pid).await?.into_inner() else {
                    println!("No deployments found");
                    return Ok(());
                };
                eprintln!("Getting logs from: {}", current.id);
                current.id
            };
            client.get_deployment_logs(pid, &id).await?
        };
        match self.output_mode {
            OutputMode::Normal => {
                let logs = r.into_inner().logs;
                for log in logs {
                    if args.raw {
                        println!("{}", log.line);
                    } else {
                        println!("{log}");
                    }
                }
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    async fn deployments_list(&self, page: u32, limit: u32, table_args: TableArgs) -> Result<()> {
        if limit == 0 {
            warn!("Limit is set to 0, no deployments will be listed.");
            return Ok(());
        }
        let client = self.client.as_ref().unwrap();
        let pid = self.ctx.project_id();

        // fetch one additional to know if there is another page available
        let limit = limit + 1;
        let (deployments, raw_json) = client
            .get_deployments(pid, page as i32, limit as i32)
            .await?
            .into_parts();
        let mut deployments = deployments.deployments;
        let page_hint = if deployments.len() == limit as usize {
            // hide the extra one and show hint instead
            deployments.pop();
            true
        } else {
            false
        };
        match self.output_mode {
            OutputMode::Normal => {
                let table = deployments_table(&deployments, table_args.raw);
                println!("{}", format!("Deployments in project '{}'", pid).bold());
                println!("{table}");
                if page_hint {
                    println!("View the next page using `--page {}`", page + 1);
                }
            }
            OutputMode::Json => {
                println!("{}", raw_json);
            }
        }

        Ok(())
    }

    async fn deployment_get(&self, deployment_id: Option<String>) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let pid = self.ctx.project_id();

        let deployment = match deployment_id {
            Some(id) => {
                let r = client.get_deployment(pid, &id).await?;
                if self.output_mode == OutputMode::Json {
                    println!("{}", r.raw_json);
                    return Ok(());
                }
                r.into_inner()
            }
            None => {
                let r = client.get_current_deployment(pid).await?;
                if self.output_mode == OutputMode::Json {
                    println!("{}", r.raw_json);
                    return Ok(());
                }

                let Some(d) = r.into_inner() else {
                    println!("No deployment found");
                    return Ok(());
                };
                d
            }
        };

        println!("{}", deployment.to_string_colored());

        Ok(())
    }

    async fn deployment_redeploy(
        &self,
        deployment_id: Option<String>,
        tracking_args: DeploymentTrackingArgs,
    ) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        let pid = self.ctx.project_id();
        let deployment_id = match deployment_id {
            Some(id) => id,
            None => {
                let d = client.get_current_deployment(pid).await?.into_inner();
                let Some(d) = d else {
                    println!("No deployment found");
                    return Ok(());
                };
                d.id
            }
        };
        let (deployment, raw_json) = client.redeploy(pid, &deployment_id).await?.into_parts();

        if tracking_args.no_follow {
            match self.output_mode {
                OutputMode::Normal => {
                    println!("{}", deployment.to_string_colored());
                }
                OutputMode::Json => {
                    println!("{}", raw_json);
                }
            }
            return Ok(());
        }

        self.track_deployment_status_and_print_logs_on_fail(pid, &deployment.id, tracking_args.raw)
            .await
    }

    async fn resources_list(&self, table_args: TableArgs, show_secrets: bool) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let pid = self.ctx.project_id();
        let r = client.get_service_resources(pid).await?;

        match self.output_mode {
            OutputMode::Normal => {
                let table = get_resource_tables(
                    r.into_inner().resources.as_slice(),
                    pid,
                    table_args.raw,
                    show_secrets,
                );
                println!("{table}");
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    async fn resource_delete(&self, resource_type: &ResourceType, no_confirm: bool) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        if !no_confirm {
            eprintln!(
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
                return Ok(());
            }
        }

        let msg = client
            .delete_service_resource(self.ctx.project_id(), resource_type)
            .await?
            .into_inner();
        println!("{msg}");

        eprintln!(
            "{}",
            formatdoc! {"
                Note:
                    Remember to remove the resource annotation from your #[shuttle_runtime::main] function.
                    Otherwise, it will be provisioned again during the next deployment."
            }
            .yellow(),
        );

        Ok(())
    }

    async fn resource_dump(&self, resource_type: &ResourceType) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        let bytes = client
            .dump_service_resource(self.ctx.project_id(), resource_type)
            .await?;
        std::io::stdout()
            .write_all(&bytes)
            .context("writing output to stdout")?;

        Ok(())
    }

    async fn list_certificates(&self, table_args: TableArgs) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let r = client.list_certificates(self.ctx.project_id()).await?;

        match self.output_mode {
            OutputMode::Normal => {
                let table =
                    get_certificates_table(r.into_inner().certificates.as_ref(), table_args.raw);
                println!("{table}");
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }
    async fn add_certificate(&self, domain: String) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let r = client
            .add_certificate(self.ctx.project_id(), domain.clone())
            .await?;

        match self.output_mode {
            OutputMode::Normal => {
                println!("Added certificate for {}", r.into_inner().subject);
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }
    async fn delete_certificate(&self, domain: String, no_confirm: bool) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        if !no_confirm {
            eprintln!(
                "{}",
                formatdoc!(
                    "
                WARNING:
                    Delete the certificate for {}?",
                    domain
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
                return Ok(());
            }
        }

        let msg = client
            .delete_certificate(self.ctx.project_id(), domain.clone())
            .await?
            .into_inner();
        println!("{msg}");

        Ok(())
    }

    fn get_secrets(
        args: &SecretsArgs,
        workspace_root: &Path,
        dev: bool,
    ) -> Result<Option<HashMap<String, String>>> {
        // Look for a secrets file, first in the command args, then in the root of the workspace.
        let files: &[PathBuf] = if dev {
            &[
                workspace_root.join("Secrets.dev.toml"),
                workspace_root.join("Secrets.toml"),
            ]
        } else {
            &[workspace_root.join("Secrets.toml")]
        };
        let secrets_file = args.secrets.as_ref().or_else(|| {
            files
                .iter()
                .find(|&secrets_file| secrets_file.exists() && secrets_file.is_file())
        });

        let Some(secrets_file) = secrets_file else {
            trace!("No secrets file was found");
            return Ok(None);
        };

        trace!("Loading secrets from {}", secrets_file.display());
        let Ok(secrets_str) = fs::read_to_string(secrets_file) else {
            tracing::warn!("Failed to read secrets file, no secrets were loaded");
            return Ok(None);
        };

        let secrets = toml::from_str::<HashMap<String, String>>(&secrets_str)
            .context("parsing secrets file")?;
        trace!(keys = ?secrets.keys(), "Loaded secrets");

        Ok(Some(secrets))
    }

    async fn build(&self, build_args: &BuildArgs) -> Result<()> {
        eprintln!("WARN: The build command is EXPERIMENTAL. Please submit feedback on GitHub or Discord if you encounter issues.");
        if let Some(path) = build_args.output_archive.as_ref() {
            let archive = self.make_archive()?;
            eprintln!("Writing archive to {}", path.display());
            fs::write(path, archive).context("writing archive")?;
            Ok(())
        } else if build_args.inner.docker {
            self.local_docker_build(&build_args.inner).await
        } else {
            self.local_build(&build_args.inner).await.map(|_| ())
        }
    }

    async fn local_build(&self, build_args: &BuildArgsShared) -> Result<BuiltService> {
        let project_directory = self.ctx.project_directory();

        if !build_args.quiet {
            cargo_green_eprintln("Building", project_directory.display());
        }

        let quiet = build_args.quiet;
        cargo_build(project_directory.to_owned(), build_args.release, quiet).await
    }

    fn find_available_port(run_args: &mut RunArgs) {
        let original_port = run_args.port;
        for port in (run_args.port..=u16::MAX).step_by(10) {
            if !portpicker::is_free_tcp(port) {
                continue;
            }
            run_args.port = port;
            break;
        }

        if run_args.port != original_port {
            eprintln!(
                "Port {} is already in use. Using port {}.",
                original_port, run_args.port,
            )
        };
    }

    async fn local_run(&self, mut run_args: RunArgs, debug: bool) -> Result<()> {
        let project_name = self.ctx.project_name().to_owned();
        let project_directory = self.ctx.project_directory();

        trace!("starting a local run with args: {run_args:?}");

        // Handle bacon mode
        if run_args.build_args.bacon {
            cargo_green_eprintln(
                "Starting",
                format!("{} in watch mode using bacon", project_name),
            );
            eprintln!();
            return bacon::run_bacon(project_directory).await;
        }

        if run_args.build_args.docker {
            eprintln!("WARN: Local run with --docker is EXPERIMENTAL. Please submit feedback on GitHub or Discord if you encounter issues.");
        }

        let secrets = Shuttle::get_secrets(&run_args.secret_args, project_directory, true)?
            .unwrap_or_default();
        Shuttle::find_available_port(&mut run_args);

        let s_re = if !run_args.build_args.docker {
            let service = self.local_build(&run_args.build_args).await?;
            trace!(path = ?service.executable_path, "runtime executable");
            if let Some(warning) = check_and_warn_runtime_version(&service.executable_path).await? {
                eprint!("{}", warning);
            }
            let runtime_executable = service.executable_path.clone();

            Some((service, runtime_executable))
        } else {
            self.local_docker_build(&run_args.build_args).await?;
            None
        };

        let api_port = portpicker::pick_unused_port()
            .expect("failed to find available port for local provisioner server");
        let api_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), api_port);
        let healthz_port = portpicker::pick_unused_port()
            .expect("failed to find available port for runtime health check");
        let ip = if run_args.external {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };

        let state = Arc::new(ProvApiState {
            project_name: project_name.clone(),
            secrets,
        });
        tokio::spawn(async move { ProvisionerServer::run(state, &api_addr).await });

        let mut envs = vec![
            ("SHUTTLE_BETA", "true".to_owned()),
            ("SHUTTLE_PROJECT_ID", "proj_LOCAL".to_owned()),
            ("SHUTTLE_PROJECT_NAME", project_name.clone()),
            ("SHUTTLE_ENV", Environment::Local.to_string()),
            ("SHUTTLE_RUNTIME_IP", ip.to_string()),
            ("SHUTTLE_RUNTIME_PORT", run_args.port.to_string()),
            ("SHUTTLE_HEALTHZ_PORT", healthz_port.to_string()),
            ("SHUTTLE_API", format!("http://127.0.0.1:{}", api_port)),
        ];
        // Use a nice debugging tracing level if user does not provide their own
        if debug && std::env::var("RUST_LOG").is_err() {
            envs.push(("RUST_LOG", "info,shuttle=trace,reqwest=debug".to_owned()));
        } else if run_args.build_args.quiet && std::env::var("RUST_LOG").is_err() {
            envs.push(("RUST_LOG", "info,shuttle=error".to_owned()));
        } else if let Ok(v) = std::env::var("RUST_LOG") {
            // forward the RUST_LOG var into the container if using docker
            envs.push(("RUST_LOG", v));
        }

        let name = format!("shuttle-run-{project_name}");
        let mut child = if run_args.build_args.docker {
            let image = format!("shuttle-build-{project_name}");
            if !run_args.build_args.quiet {
                eprintln!();
                cargo_green_eprintln(
                    "Starting",
                    format!("{} on http://{}:{}", image, ip, run_args.port),
                );
                eprintln!();
            }
            info!(image, "Spawning 'docker run' process");
            let mut docker = tokio::process::Command::new("docker");
            docker
                .arg("run")
                // the kill on docker run does not work as well as manual docker stop after quitting,
                // but this is good to have regardless
                .arg("--rm")
                .arg("--network")
                .arg("host")
                .arg("--name")
                .arg(&name);
            for (k, v) in envs {
                docker.arg("--env").arg(format!("{k}={v}"));
            }

            docker
                .arg(&image)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .context("spawning 'docker run' process")?
        } else {
            let (service, runtime_executable) = s_re.context("developer skill issue")?;
            if !run_args.build_args.quiet {
                eprintln!();
                cargo_green_eprintln(
                    "Starting",
                    format!("{} on http://{}:{}", service.target_name, ip, run_args.port),
                );
                eprintln!();
            }
            info!(
                path = %runtime_executable.display(),
                "Spawning runtime process",
            );
            tokio::process::Command::new(
                dunce::canonicalize(runtime_executable)
                    .context("canonicalize path of executable")?,
            )
            .current_dir(&service.workspace_path)
            .envs(envs)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("spawning runtime process")?
        };

        // Start background tasks for reading child's stdout and stderr
        let raw = run_args.raw;
        let mut stdout_reader = BufReader::new(
            child
                .stdout
                .take()
                .context("child process did not have a handle to stdout")?,
        )
        .lines();
        tokio::spawn(async move {
            while let Some(line) = stdout_reader.next_line().await.unwrap() {
                if raw {
                    println!("{}", line);
                } else {
                    let log_item = LogItem::new(Utc::now(), "app".to_owned(), line);
                    println!("{log_item}");
                }
            }
        });
        let mut stderr_reader = BufReader::new(
            child
                .stderr
                .take()
                .context("child process did not have a handle to stderr")?,
        )
        .lines();
        tokio::spawn(async move {
            while let Some(line) = stderr_reader.next_line().await.unwrap() {
                if raw {
                    println!("{}", line);
                } else {
                    let log_item = LogItem::new(Utc::now(), "app".to_owned(), line);
                    println!("{log_item}");
                }
            }
        });

        // Start background task for simulated health check
        tokio::spawn(async move {
            loop {
                // ECS health check runs ever 5s
                tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

                tracing::trace!("Health check against runtime");
                if let Err(e) = reqwest::get(format!("http://127.0.0.1:{}/", healthz_port)).await {
                    tracing::trace!("Health check against runtime failed: {e}");
                }
            }
        });

        #[cfg(target_family = "unix")]
        let exit_result = {
            let mut sigterm_notif =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Can not get the SIGTERM signal receptor");
            let mut sigint_notif =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                    .expect("Can not get the SIGINT signal receptor");
            tokio::select! {
                exit_result = child.wait() => {
                    Some(exit_result)
                }
                _ = sigterm_notif.recv() => {
                    eprintln!("Received SIGTERM.");
                    None
                },
                _ = sigint_notif.recv() => {
                    eprintln!("Received SIGINT.");
                    None
                }
            }
        };
        #[cfg(target_family = "windows")]
        let exit_result = {
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
                exit_result = child.wait() => {
                    Some(exit_result)
                }
                _ = ctrl_break_notif.recv() => {
                    eprintln!("Received ctrl-break.");
                    None
                },
                _ = ctrl_c_notif.recv() => {
                    eprintln!("Received ctrl-c.");
                    None
                },
                _ = ctrl_close_notif.recv() => {
                    eprintln!("Received ctrl-close.");
                    None
                },
                _ = ctrl_logoff_notif.recv() => {
                    eprintln!("Received ctrl-logoff.");
                    None
                },
                _ = ctrl_shutdown_notif.recv() => {
                    eprintln!("Received ctrl-shutdown.");
                    None
                }
            }
        };
        match exit_result {
            Some(Ok(exit_status)) => {
                bail!(
                    "Runtime process exited with code {}",
                    exit_status.code().unwrap_or_default()
                );
            }
            Some(Err(e)) => {
                bail!("Failed to wait for runtime process to exit: {e}");
            }
            None => {
                eprintln!("Stopping runtime.");
                child.kill().await?;
                if run_args.build_args.docker {
                    let status = tokio::process::Command::new("docker")
                        .arg("stop")
                        .arg(name)
                        .kill_on_drop(true)
                        .stdout(Stdio::null())
                        .spawn()
                        .context("spawning 'docker stop'")?
                        .wait()
                        .await
                        .context("waiting for 'docker stop'")?;

                    if !status.success() {
                        eprintln!("WARN: 'docker stop' failed");
                    }
                }
            }
        }

        Ok(())
    }

    async fn local_docker_build(&self, build_args: &BuildArgsShared) -> Result<()> {
        let project_name = self.ctx.project_name().to_owned();
        let project_directory = self.ctx.project_directory();

        let metadata = cargo_metadata(project_directory)?;
        let rust_build_args = gather_rust_build_args(&metadata)?;

        cargo_green_eprintln("Building", format!("{} with docker", project_name));

        let tempdir = tempfile::Builder::new()
            .prefix("shuttle-build-")
            .tempdir()?
            .keep();
        tracing::debug!("Building in {}", tempdir.display());

        let build_files = self.gather_build_files()?;
        if build_files.is_empty() {
            error!("No files included in build. Aborting...");
            bail!("No files included in build");
        }

        // make sure this file exists
        tracing::debug!("Creating prebuild script file");
        fs::write(tempdir.join("shuttle_prebuild.sh"), "")?;
        for (path, name) in build_files {
            let dest = tempdir.join(&name);
            tracing::debug!("Copying {} to tempdir", name.display());
            fs::create_dir_all(dest.parent().expect("destination to not be the root"))?;
            fs::copy(path, dest)?;
        }
        tracing::debug!("Removing any .dockerignore file");
        // remove .dockerignore to not interfere
        let _ = fs::remove_file(tempdir.join(".dockerignore"));

        // TODO?: Support custom shuttle.Dockerfile
        let dockerfile = tempdir.join("__shuttle.Dockerfile");
        tracing::debug!("Writing dockerfile to {}", dockerfile.display());
        fs::write(&dockerfile, render_rust_dockerfile(&rust_build_args))?;

        let mut docker_cmd = tokio::process::Command::new("docker");
        docker_cmd
            .arg("buildx")
            .arg("build")
            .arg("--file")
            .arg(dockerfile)
            .arg("--tag")
            .arg(format!("shuttle-build-{project_name}"));
        if let Some(ref tag) = build_args.tag {
            docker_cmd.arg("--tag").arg(tag);
        }

        let docker = docker_cmd.arg(tempdir).kill_on_drop(true).spawn();

        let result = docker
            .context("spawning docker build command")?
            .wait()
            .await
            .context("waiting for docker build to exit")?;
        if !result.success() {
            bail!("Docker build error");
        }

        cargo_green_eprintln("Finished", "building with docker");

        Ok(())
    }

    async fn deploy(&mut self, args: DeployArgs) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let project_directory = self.ctx.project_directory();

        let secrets = Shuttle::get_secrets(&args.secret_args, project_directory, false)?;

        // Image deployment mode
        if let Some(image) = args.image {
            let pid = self.ctx.project_id();
            let deployment_req_image = DeploymentRequestImage { image, secrets };

            let (deployment, raw_json) = client
                .deploy(pid, DeploymentRequest::Image(deployment_req_image))
                .await?
                .into_parts();

            if args.tracking_args.no_follow {
                match self.output_mode {
                    OutputMode::Normal => {
                        println!("{}", deployment.to_string_colored());
                    }
                    OutputMode::Json => {
                        println!("{}", raw_json);
                    }
                }
                return Ok(());
            }

            return self
                .track_deployment_status_and_print_logs_on_fail(
                    pid,
                    &deployment.id,
                    args.tracking_args.raw,
                )
                .await;
        }

        // Build archive deployment mode
        let mut deployment_req = DeploymentRequestBuildArchive {
            secrets,
            ..Default::default()
        };
        let mut build_meta = BuildMeta::default();

        let metadata = cargo_metadata(project_directory)?;

        let rust_build_args = gather_rust_build_args(&metadata)?;
        deployment_req.build_args = Some(CommonBuildArgs::Rust(rust_build_args));

        let (_, target, _) = find_first_shuttle_package(&metadata)?;
        deployment_req.infra = parse_infra_from_code(
            &fs::read_to_string(target.src_path.as_path())
                .context("reading target file when extracting infra annotations")?,
        )
        .context("parsing infra annotations")?;

        if let Ok(repo) = Repository::discover(project_directory) {
            let repo_path = repo
                .workdir()
                .context("getting working directory of repository")?;
            let repo_path = dunce::canonicalize(repo_path)?;
            trace!(?repo_path, "found git repository");

            let dirty = is_dirty(&repo);
            build_meta.git_dirty = Some(dirty.is_err());

            let check_dirty = self.ctx.deny_dirty().is_some_and(|d| d);
            if check_dirty && !args.allow_dirty && dirty.is_err() {
                bail!(dirty.unwrap_err());
            }

            if let Ok(head) = repo.head() {
                // This is typically the name of the current branch
                // It is "HEAD" when head detached, for example when a tag is checked out
                build_meta.git_branch = head
                    .shorthand()
                    .map(|s| s.chars().take(GIT_STRINGS_MAX_LENGTH).collect());
                if let Ok(commit) = head.peel_to_commit() {
                    build_meta.git_commit_id = Some(commit.id().to_string());
                    // Summary is None if error or invalid utf-8
                    build_meta.git_commit_msg = commit
                        .summary()
                        .map(|s| s.chars().take(GIT_STRINGS_MAX_LENGTH).collect());
                }
            }
        }

        cargo_green_eprintln("Packing", "build files");
        let archive = self.make_archive()?;

        if let Some(path) = args.output_archive {
            eprintln!("Writing archive to {}", path.display());
            fs::write(path, archive).context("writing archive")?;

            return Ok(());
        }

        // TODO: upload secrets separately

        let pid = self.ctx.project_id();

        cargo_green_eprintln("Uploading", "build archive");
        let arch = client.upload_archive(pid, archive).await?.into_inner();
        deployment_req.archive_version_id = arch.archive_version_id;
        deployment_req.build_meta = Some(build_meta);

        cargo_green_eprintln("Creating", "deployment");
        let (deployment, raw_json) = client
            .deploy(
                pid,
                DeploymentRequest::BuildArchive(Box::new(deployment_req)),
            )
            .await?
            .into_parts();

        if args.tracking_args.no_follow {
            match self.output_mode {
                OutputMode::Normal => {
                    println!("{}", deployment.to_string_colored());
                }
                OutputMode::Json => {
                    println!("{}", raw_json);
                }
            }
            return Ok(());
        }

        self.track_deployment_status_and_print_logs_on_fail(
            pid,
            &deployment.id,
            args.tracking_args.raw,
        )
        .await
    }

    /// Returns true if the deployment failed
    async fn track_deployment_status(&self, pid: &str, id: &str) -> Result<bool> {
        let client = self.client.as_ref().unwrap();
        let failed = wait_with_spinner(2000, |_, pb| async move {
            let (deployment, raw_json) = client.get_deployment(pid, id).await?.into_parts();

            let state = deployment.state.clone();
            match self.output_mode {
                OutputMode::Normal => {
                    pb.set_message(deployment.to_string_summary_colored());
                }
                OutputMode::Json => {
                    println!("{}", raw_json);
                }
            }
            let failed = state == DeploymentState::Failed;
            let cleanup = move || {
                match self.output_mode {
                    OutputMode::Normal => {
                        eprintln!("{}", deployment.to_string_colored());
                    }
                    OutputMode::Json => {
                        // last deployment response already printed
                    }
                }
                failed
            };
            match state {
                // non-end states
                DeploymentState::Pending
                | DeploymentState::Building
                | DeploymentState::InProgress => Ok(None),
                // end states
                DeploymentState::Running
                | DeploymentState::Stopped
                | DeploymentState::Stopping
                | DeploymentState::Unknown(_)
                | DeploymentState::Failed => Ok(Some(cleanup)),
            }
        })
        .await?;

        Ok(failed)
    }

    async fn track_deployment_status_and_print_logs_on_fail(
        &self,
        proj_id: &str,
        depl_id: &str,
        raw: bool,
    ) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let failed = self.track_deployment_status(proj_id, depl_id).await?;
        if failed {
            let r = client.get_deployment_logs(proj_id, depl_id).await?;
            match self.output_mode {
                OutputMode::Normal => {
                    let logs = r.into_inner().logs;
                    for log in logs {
                        if raw {
                            println!("{}", log.line);
                        } else {
                            println!("{log}");
                        }
                    }
                }
                OutputMode::Json => {
                    println!("{}", r.raw_json);
                }
            }
            bail!("Deployment failed");
        }

        Ok(())
    }

    async fn project_create(&self, name: Option<String>) -> Result<()> {
        let Some(ref name) = name else {
            bail!("Provide a project name with '--name <name>'");
        };

        let client = self.client.as_ref().unwrap();
        let r = client.create_project(name).await?;

        match self.output_mode {
            OutputMode::Normal => {
                let project = r.into_inner();
                println!("Created project '{}' with id {}", project.name, project.id);
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    async fn project_rename(&self, name: String) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        let r = client
            .update_project(
                self.ctx.project_id(),
                ProjectUpdateRequest {
                    name: Some(name),
                    ..Default::default()
                },
            )
            .await?;

        match self.output_mode {
            OutputMode::Normal => {
                let project = r.into_inner();
                println!("Renamed project {} to '{}'", project.id, project.name);
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    async fn projects_list(&self, table_args: TableArgs) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let r = client.get_projects_list().await?;

        match self.output_mode {
            OutputMode::Normal => {
                let all_projects = r.into_inner().projects;
                // partition by team id and print separate tables
                let mut all_projects_map = BTreeMap::new();
                for proj in all_projects {
                    all_projects_map
                        .entry(proj.team_id.clone())
                        .or_insert_with(Vec::new)
                        .push(proj);
                }
                for (team_id, projects) in all_projects_map {
                    println!(
                        "{}",
                        if let Some(team_id) = team_id {
                            format!("Team {} projects", team_id)
                        } else {
                            "Personal Projects".to_owned()
                        }
                        .bold()
                    );
                    println!("{}\n", get_projects_table(&projects, table_args.raw));
                }
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    async fn project_status(&self) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let r = client.get_project(self.ctx.project_id()).await?;

        match self.output_mode {
            OutputMode::Normal => {
                print!("{}", r.into_inner().to_string_colored());
            }
            OutputMode::Json => {
                println!("{}", r.raw_json);
            }
        }

        Ok(())
    }

    async fn project_delete(&self, no_confirm: bool) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let pid = self.ctx.project_id();

        if !no_confirm {
            // check that the project exists, and look up the name
            let proj = client.get_project(pid).await?.into_inner();
            eprintln!(
                "{}",
                formatdoc!(
                    r#"
                    WARNING:
                        Are you sure you want to delete '{}' ({})?
                        This will...
                        - Shut down your service
                        - Delete any databases and secrets in this project
                        - Delete any custom domains linked to this project
                        This action is permanent."#,
                    proj.name,
                    pid,
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
                return Ok(());
            }
        }

        let res = client.delete_project(pid).await?.into_inner();

        println!("{res}");

        Ok(())
    }

    /// Find list of all files to include in a build, ready for placing in a zip archive
    fn gather_build_files(&self) -> Result<BTreeMap<PathBuf, PathBuf>> {
        let include_patterns = self.ctx.include();
        let project_directory = self.ctx.project_directory();

        //
        // Mixing include and exclude overrides messes up the .ignore and .gitignore etc,
        // therefore these "ignore" walk and the "include" walk are separate.
        //
        let mut entries = Vec::new();

        // Default excludes
        let ignore_overrides = OverrideBuilder::new(project_directory)
            .add("!.git/")
            .context("adding override `!.git/`")?
            .add("!target/")
            .context("adding override `!target/`")?
            .build()
            .context("building archive override rules")?;
        for r in WalkBuilder::new(project_directory)
            .hidden(false)
            .overrides(ignore_overrides)
            .build()
        {
            entries.push(r.context("list dir entry")?.into_path())
        }

        // User provided includes
        let mut globs = GlobSetBuilder::new();
        if let Some(rules) = include_patterns {
            for r in rules {
                globs.add(Glob::new(r.as_str()).context(format!("parsing glob pattern {:?}", r))?);
            }
        }

        // Find the files
        let globs = globs.build().context("glob glob")?;
        for entry in walkdir::WalkDir::new(project_directory) {
            let path = entry.context("list dir")?.into_path();
            if globs.is_match(
                path.strip_prefix(project_directory)
                    .context("strip prefix of path")?,
            ) {
                entries.push(path);
            }
        }

        let mut archive_files = BTreeMap::new();
        for path in entries {
            // It's not possible to add a directory to an archive
            if path.is_dir() {
                trace!("Skipping {:?}: is a directory", path);
                continue;
            }
            // symlinks == chaos
            if path.is_symlink() {
                trace!("Skipping {:?}: is a symlink", path);
                continue;
            }

            // zip file puts all files in root
            let name = path
                .strip_prefix(project_directory)
                .context("strip prefix of path")?
                .to_owned();

            archive_files.insert(path, name);
        }

        Ok(archive_files)
    }

    fn make_archive(&self) -> Result<Vec<u8>> {
        let archive_files = self.gather_build_files()?;
        if archive_files.is_empty() {
            bail!("No files included in build");
        }

        let bytes = {
            debug!("making zip archive");
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            for (path, name) in archive_files {
                debug!("Packing {path:?}");

                // windows things
                let name = name.to_str().expect("valid filename").replace('\\', "/");
                zip.start_file(name, FileOptions::<()>::default())?;

                let mut b = Vec::new();
                fs::File::open(path)?.read_to_end(&mut b)?;
                zip.write_all(&b)?;
            }
            let r = zip.finish().context("finish encoding zip archive")?;

            r.into_inner()
        };
        debug!("Archive size: {} bytes", bytes.len());

        Ok(bytes)
    }
}

/// Calls async function `f` in a loop with `millis` sleep between iterations,
/// providing iteration count and reference to update the progress bar.
/// `f` returns Some with a cleanup function if done.
/// The cleanup function is called after teardown of progress bar,
/// and its return value is returned from here.
async fn wait_with_spinner<Fut, C, O>(
    millis: u64,
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
        sleep(Duration::from_millis(millis)).await;
    };
    progress_bar.finish_and_clear();

    Ok(cleanup())
}

fn create_spinner() -> ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.enable_steady_tick(std::time::Duration::from_millis(250));
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

#[cfg(test)]
mod tests {
    use zip::ZipArchive;

    use crate::args::ProjectArgs;
    use crate::Shuttle;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;

    pub fn path_from_workspace_root(path: &str) -> PathBuf {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path);

        dunce::canonicalize(path).unwrap()
    }

    async fn get_archive_entries(project_args: ProjectArgs) -> Vec<String> {
        let mut shuttle = Shuttle::new(crate::Binary::Shuttle, None).unwrap();
        shuttle
            .load_project_id(&project_args, false, false)
            .await
            .unwrap();

        let archive = shuttle.make_archive().unwrap();

        let mut zip = ZipArchive::new(Cursor::new(archive)).unwrap();
        (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect()
    }

    #[tokio::test]
    async fn make_archive_respect_rules() {
        let working_directory = fs::canonicalize(path_from_workspace_root(
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
            name: None,
            id: Some("proj_archiving-test".to_owned()),
        };
        let mut entries = get_archive_entries(project_args.clone()).await;
        entries.sort();

        let expected = vec![
            ".gitignore",
            ".ignore",
            "Cargo.toml",
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
        ];
        assert_eq!(entries, expected);
    }

    #[tokio::test]
    async fn finds_workspace_root() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
            id: None,
        };

        assert_eq!(
            project_args.workspace_path().unwrap(),
            path_from_workspace_root("examples/axum/hello-world")
        );
    }
}
