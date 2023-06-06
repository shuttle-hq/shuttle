mod args;
mod client;
pub mod config;
mod init;
mod provisioner_server;

use args::LogoutArgs;
use indicatif::ProgressBar;
use shuttle_common::claims::{ClaimService, InjectPropagation};
use shuttle_common::models::deployment::get_deployments_table;
use shuttle_common::models::project::IDLE_MINUTES;
use shuttle_common::models::resource::get_resources_table;
use shuttle_common::project::ProjectName;
use shuttle_common::{resource, ApiKey};
use shuttle_proto::runtime::runtime_client::RuntimeClient;
use shuttle_proto::runtime::{self, LoadRequest, StartRequest, StopRequest, SubscribeLogsRequest};

use tokio::process::Child;
use tokio::task::JoinHandle;
use tonic::transport::Channel;
use tonic::Status;

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{read_to_string, File};
use std::io::stdout;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use std::process::exit;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
pub use args::{Args, Command, DeployArgs, InitArgs, LoginArgs, ProjectArgs, RunArgs};
use cargo_metadata::Message;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use config::RequestContext;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Password};
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{StreamExt, TryFutureExt};
use git2::{Repository, StatusOptions};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use shuttle_common::models::{project, secret};
use shuttle_service::builder::{build_workspace, BuiltService};
use std::fmt::Write;
use strum::IntoEnumIterator;
use tar::Builder;
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

use crate::args::{DeploymentCommand, ProjectCommand, ProjectStartArgs, ResourceCommand};
use crate::client::Client;
use crate::provisioner_server::LocalProvisioner;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
const SHUTTLE_LOGIN_URL: &str = "https://shuttle.rs/login";
const SHUTTLE_GH_ISSUE_URL: &str = "https://github.com/shuttle-hq/shuttle/issues/new";

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
                )
                | Command::Stop
                | Command::Clean
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
            Command::Logout(logout_args) => self.logout(logout_args).await,
            Command::Feedback => self.feedback().await,
            Command::Run(run_args) => self.local_run(run_args).await,
            Command::Deploy(deploy_args) => {
                return self.deploy(&self.client()?, deploy_args).await;
            }
            Command::Status => self.status(&self.client()?).await,
            Command::Logs { id, latest, follow } => {
                self.logs(&self.client()?, id, latest, follow).await
            }
            Command::Deployment(DeploymentCommand::List { page, limit }) => {
                self.deployments_list(&self.client()?, page, limit).await
            }
            Command::Deployment(DeploymentCommand::Status { id }) => {
                self.deployment_get(&self.client()?, id).await
            }
            Command::Resource(ResourceCommand::List) => self.resources_list(&self.client()?).await,
            Command::Stop => self.stop(&self.client()?).await,
            Command::Clean => self.clean(&self.client()?).await,
            Command::Secrets => self.secrets(&self.client()?).await,
            Command::Project(ProjectCommand::Start(ProjectStartArgs { idle_minutes })) => {
                self.project_create(&self.client()?, idle_minutes).await
            }
            Command::Project(ProjectCommand::Restart(ProjectStartArgs { idle_minutes })) => {
                self.project_recreate(&self.client()?, idle_minutes).await
            }
            Command::Project(ProjectCommand::Status { follow }) => {
                self.project_status(&self.client()?, follow).await
            }
            Command::Project(ProjectCommand::List { page, limit }) => {
                self.projects_list(&self.client()?, page, limit).await
            }
            Command::Project(ProjectCommand::Stop) => self.project_delete(&self.client()?).await,
        }
        .map(|_| CommandOutcome::Ok)
    }

    fn client(&self) -> Result<Client> {
        let mut client = Client::new(self.ctx.api_url());
        client.set_api_key(self.ctx.api_key()?);
        Ok(client)
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
            } else if args.login_args.api_key.is_some() {
                self.login(args.login_args.clone()).await?;
            } else if args.create_env {
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
            args::parse_init_path(OsString::from(directory_str))?
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
                let frameworks = init::Template::iter().collect::<Vec<_>>();
                let index = FuzzySelect::with_theme(&theme)
                    .items(&frameworks)
                    .default(0)
                    .interact()?;
                println!();
                frameworks[index]
            }
        };

        // 5. Initialize locally
        init::cargo_generate(path.clone(), project_args.name.clone().unwrap(), framework)?;
        println!();

        // 6. Confirm that the user wants to create the project environment on Shuttle
        let should_create_environment = if !interactive {
            args.create_env
        } else if args.create_env {
            true
        } else {
            let should_create = Confirm::with_theme(&theme)
                .with_prompt("Do you want to create the project environment on Shuttle?")
                .default(true)
                .interact()?;

            println!();
            should_create
        };

        if should_create_environment {
            // Set the project working directory path to the init path,
            // so `load_project` is ran with the correct project path
            project_args.working_directory = path;

            self.load_project(&mut project_args)?;
            self.project_create(&self.client()?, IDLE_MINUTES).await?;
        } else {
            println!(
                "Run `cargo shuttle project start` to create a project environment on Shuttle."
            );
        }

        Ok(())
    }

    pub fn load_project(&mut self, project_args: &mut ProjectArgs) -> Result<()> {
        trace!("loading project arguments: {project_args:?}");

        self.ctx.load_local(project_args)
    }

    /// Provide feedback on GitHub.
    async fn feedback(&self) -> Result<()> {
        let _ = webbrowser::open(SHUTTLE_GH_ISSUE_URL);
        println!("If your browser did not open automatically, go to {SHUTTLE_GH_ISSUE_URL}");

        Ok(())
    }

    /// Log in with the given API key or after prompting the user for one.
    async fn login(&mut self, login_args: LoginArgs) -> Result<()> {
        let api_key_str = match login_args.api_key {
            Some(api_key) => api_key,
            None => {
                let _ = webbrowser::open(SHUTTLE_LOGIN_URL);
                println!("If your browser did not automatically open, go to {SHUTTLE_LOGIN_URL}");

                Password::with_theme(&ColorfulTheme::default())
                    .with_prompt("API key")
                    .validate_with(|input: &String| ApiKey::parse(input).map(|_| {}))
                    .interact()?
            }
        };

        let api_key = ApiKey::parse(&api_key_str)?;

        self.ctx.set_api_key(api_key)?;

        Ok(())
    }

    async fn logout(&mut self, logout_args: LogoutArgs) -> Result<()> {
        if logout_args.reset_api_key {
            self.reset_api_key(&self.client()?).await?;
            println!("Successfully reset the API key.");
            println!(" -> Go to {SHUTTLE_LOGIN_URL} to get a new one.\n");
        }
        self.ctx.clear_api_key()?;
        println!("Successfully logged out of shuttle.");

        Ok(())
    }

    async fn reset_api_key(&self, client: &Client) -> Result<()> {
        client.reset_api_key().await.and_then(|res| {
            if res.status().is_success() {
                Ok(())
            } else {
                Err(anyhow!("Resetting API key failed."))
            }
        })
    }

    async fn stop(&self, client: &Client) -> Result<()> {
        let proj_name = self.ctx.project_name();
        let mut service = client.stop_service(proj_name).await?;

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
        let summary = client.get_service(self.ctx.project_name()).await?;

        println!("{summary}");

        Ok(())
    }

    async fn secrets(&self, client: &Client) -> Result<()> {
        let secrets = client.get_secrets(self.ctx.project_name()).await?;
        let table = secret::get_table(&secrets);

        println!("{table}");

        Ok(())
    }

    async fn clean(&self, client: &Client) -> Result<()> {
        let lines = client.clean_project(self.ctx.project_name()).await?;

        for line in lines {
            println!("{line}");
        }

        println!("Cleaning done!");

        Ok(())
    }

    async fn logs(
        &self,
        client: &Client,
        id: Option<Uuid>,
        latest: bool,
        follow: bool,
    ) -> Result<()> {
        let id = if let Some(id) = id {
            id
        } else {
            let proj_name = self.ctx.project_name();

            if latest {
                // Find latest deployment (not always an active one)
                let deployments = client.get_deployments(proj_name, 0, u32::MAX).await?;
                let most_recent = deployments.last().context(format!(
                    "Could not find any deployments for '{proj_name}'. Try passing a deployment ID manually",
                ))?;

                most_recent.id
            } else if let Some(deployment) = client.get_service(proj_name).await?.deployment {
                // Active deployment
                deployment.id
            } else {
                bail!(
                    "Could not find a running deployment for '{proj_name}'. Try with '--latest', or pass a deployment ID manually"
                );
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

    async fn deployments_list(&self, client: &Client, page: u32, limit: u32) -> Result<()> {
        if limit == 0 {
            println!();
            return Ok(());
        }

        let proj_name = self.ctx.project_name();
        let deployments = client.get_deployments(proj_name, page, limit).await?;
        let table = get_deployments_table(&deployments, proj_name.as_str(), page);

        println!("{table}");
        println!("Run `cargo shuttle logs <id>` to get logs for a given deployment.");

        Ok(())
    }

    async fn deployment_get(&self, client: &Client, deployment_id: Uuid) -> Result<()> {
        let deployment = client
            .get_deployment_details(self.ctx.project_name(), &deployment_id)
            .await?;

        println!("{deployment}");

        Ok(())
    }

    async fn resources_list(&self, client: &Client) -> Result<()> {
        let resources = client
            .get_service_resources(self.ctx.project_name())
            .await?;
        let table = get_resources_table(&resources, self.ctx.project_name().as_str());

        println!("{table}");

        Ok(())
    }

    async fn spin_local_runtime(
        run_args: &RunArgs,
        service: &BuiltService,
        provisioner_server: &JoinHandle<Result<(), tonic::transport::Error>>,
        i: u16,
        provisioner_port: u16,
    ) -> Result<
        Option<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )>,
    > {
        let BuiltService {
            executable_path,
            is_wasm,
            working_directory,
            ..
        } = service.clone();

        trace!("loading secrets");
        let secrets_path = if working_directory.join("Secrets.dev.toml").exists() {
            working_directory.join("Secrets.dev.toml")
        } else {
            working_directory.join("Secrets.toml")
        };

        let secrets: HashMap<String, String> = if let Ok(secrets_str) = read_to_string(secrets_path)
        {
            let secrets: HashMap<String, String> =
                secrets_str.parse::<toml::Value>()?.try_into()?;

            trace!(keys = ?secrets.keys(), "available secrets");

            secrets
        } else {
            trace!("no Secrets.toml was found");
            Default::default()
        };

        let runtime_path = || {
            if is_wasm {
                let runtime_path = home::cargo_home()
                    .expect("failed to find cargo home dir")
                    .join("bin/shuttle-next");

                println!("Installing shuttle-next runtime. This can take a while...");

                if cfg!(debug_assertions) {
                    // Canonicalized path to shuttle-runtime for dev to work on windows

                    let path = std::fs::canonicalize(format!("{MANIFEST_DIR}/../runtime"))
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
                trace!(path = ?executable_path, "using alpha runtime");
                executable_path.clone()
            }
        };

        let (mut runtime, mut runtime_client) = runtime::start(
            is_wasm,
            runtime::StorageManagerType::WorkingDir(working_directory.to_path_buf()),
            &format!("http://localhost:{provisioner_port}"),
            None,
            run_args.port - (1 + i),
            runtime_path,
        )
        .await
        .map_err(|err| {
            provisioner_server.abort();
            err
        })?;

        let service_name = service.service_name()?;
        let load_request = tonic::Request::new(LoadRequest {
            path: executable_path
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

        let mut stream = runtime_client
            .subscribe_logs(tonic::Request::new(SubscribeLogsRequest {}))
            .or_else(|err| async {
                provisioner_server.abort();
                runtime.kill().await?;
                Err(err)
            })
            .await?
            .into_inner();

        tokio::spawn(async move {
            while let Ok(Some(log)) = stream.message().await {
                let log: shuttle_common::LogItem = log.try_into().expect("to convert log");
                println!("{log}");
            }
        });

        let addr = SocketAddr::new(
            if run_args.external {
                Ipv4Addr::new(0, 0, 0, 0)
            } else {
                Ipv4Addr::LOCALHOST
            }
            .into(),
            run_args.port + i,
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
        provisioner_server: &JoinHandle<Result<(), tonic::transport::Error>>,
    ) -> Result<(), Status> {
        match runtime {
            Some(inner) => existing_runtimes.push(inner),
            None => {
                provisioner_server.abort();
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
            portpicker::pick_unused_port().expect("unable to find available port");
        let provisioner_server = provisioner.start(SocketAddr::new(
            Ipv4Addr::LOCALHOST.into(),
            provisioner_port,
        ));

        Ok((provisioner_server, provisioner_port))
    }

    #[cfg(target_family = "unix")]
    async fn local_run(&self, run_args: RunArgs) -> Result<()> {
        let services = Shuttle::pre_local_run(self, &run_args).await?;
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
        let mut signal_received = false;
        for (i, service) in services.iter().enumerate() {
            // We must cover the case of starting multiple workspace services and receiving a signal in parallel.
            // This must stop all the existing runtimes and creating new ones.
            signal_received = tokio::select! {
                res = Shuttle::spin_local_runtime(&run_args, service, &provisioner_server, i as u16, provisioner_port) => {
                    Shuttle::add_runtime_info(res.unwrap(), &mut runtimes, &provisioner_server).await?;
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
            return Ok(());
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

        Ok(())
    }

    #[cfg(target_family = "windows")]
    async fn local_run(&self, run_args: RunArgs) -> Result<()> {
        let services = Shuttle::pre_local_run(&self, &run_args).await?;
        let (provisioner_server, provisioner_port) = Shuttle::setup_local_provisioner().await?;

        // Start all the services.
        let mut runtimes: Vec<(
            Child,
            RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        )> = Vec::new();
        for (i, service) in services.iter().enumerate() {
            Shuttle::add_runtime_info(
                Shuttle::spin_local_runtime(
                    &run_args,
                    service,
                    &provisioner_server,
                    i as u16,
                    provisioner_port,
                )
                .await?,
                &mut runtimes,
                &provisioner_server,
            )
            .await?;
        }

        for (mut rt, _) in runtimes {
            println!(
                "a service future completed with exit status: {:?}",
                rt.wait().await?.code()
            );
        }

        println!(
            "Run `cargo shuttle project start` to create a project environment on Shuttle.\n\
             Run `cargo shuttle deploy` to deploy your Shuttle service."
        );

        Ok(())
    }

    async fn deploy(&self, client: &Client, args: DeployArgs) -> Result<CommandOutcome> {
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

        loop {
            let message = stream.next().await;
            if let Some(Ok(msg)) = message {
                if let tokio_tungstenite::tungstenite::Message::Text(line) = msg {
                    let log_item: shuttle_common::LogItem =
                        serde_json::from_str(&line).expect("to parse log line");

                    match log_item.state.clone() {
                        shuttle_common::deployment::State::Queued
                        | shuttle_common::deployment::State::Building
                        | shuttle_common::deployment::State::Built
                        | shuttle_common::deployment::State::Loading => {
                            println!("{log_item}");
                        }
                        shuttle_common::deployment::State::Crashed => {
                            println!();
                            println!("{}", "Deployment crashed".red());
                            println!();
                            println!("Run the following for more details");
                            println!();
                            print!("cargo shuttle logs {}", &deployment.id);
                            println!();

                            return Ok(CommandOutcome::DeploymentFailure);
                        }
                        shuttle_common::deployment::State::Running
                        | shuttle_common::deployment::State::Completed
                        | shuttle_common::deployment::State::Stopped
                        | shuttle_common::deployment::State::Unknown => {
                            break;
                        }
                    };
                }
            } else {
                println!("Reconnecting websockets logging");
                // A wait time short enough for not much state to have changed, long enough that
                // the terminal isn't completely spammed
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                stream = client
                    .get_logs_ws(self.ctx.project_name(), &deployment.id)
                    .await?;
            }
        }

        // Temporary fix.
        // TODO: Make get_service_summary endpoint wait for a bit and see if it entered Running/Crashed state.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let deployment = client
            .get_deployment_details(self.ctx.project_name(), &deployment.id)
            .await?;

        // A deployment will only exist if there is currently one in the running state
        if deployment.state == shuttle_common::deployment::State::Running {
            let service = client.get_service(self.ctx.project_name()).await?;

            let resources = client
                .get_service_resources(self.ctx.project_name())
                .await?;
            let resources = get_resources_table(&resources, self.ctx.project_name().as_str());

            println!("{resources}{service}");

            Ok(CommandOutcome::Ok)
        } else {
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
                _ => println!(
                    "Deployment encountered an unexpected error - Please create a ticket to report this."
                ),
            }

            println!();
            println!("Run the following for more details");
            println!();
            println!("cargo shuttle logs {}", &deployment.id);
            println!();

            Ok(CommandOutcome::DeploymentFailure)
        }
    }

    async fn project_create(&self, client: &Client, idle_minutes: u64) -> Result<()> {
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
        .await?;
        println!("Run `cargo shuttle deploy` to deploy your Shuttle service.");

        Ok(())
    }

    async fn project_recreate(&self, client: &Client, idle_minutes: u64) -> Result<()> {
        self.project_delete(client).await?;
        self.project_create(client, idle_minutes).await?;

        Ok(())
    }

    async fn projects_list(&self, client: &Client, page: u32, limit: u32) -> Result<()> {
        if limit == 0 {
            println!();
            return Ok(());
        }

        let projects = client.get_projects_list(page, limit).await?;
        let projects_table = project::get_table(&projects, page);

        println!("{projects_table}");

        Ok(())
    }

    async fn project_status(&self, client: &Client, follow: bool) -> Result<()> {
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
            let project = client.get_project(self.ctx.project_name()).await?;
            println!("{project}");
        }

        Ok(())
    }

    async fn project_delete(&self, client: &Client) -> Result<()> {
        self.wait_with_spinner(
            &[
                project::State::Destroyed,
                project::State::Errored {
                    message: Default::default(),
                },
            ],
            client.delete_project(self.ctx.project_name()),
            self.ctx.project_name(),
            client,
        )
        .await?;
        println!("Run `cargo shuttle project start` to recreate project environment on Shuttle.");

        Ok(())
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
        }
        progress_bar.finish_and_clear();
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

        // Add all the entries to a map to avoid duplication of the Secrets.toml file
        // if it is in the root of the workspace.
        let mut entries = HashMap::new();

        for dir_entry in WalkBuilder::new(working_directory)
            .hidden(false)
            .overrides(overrides)
            .build()
        {
            let dir_entry = dir_entry.context("get directory entry")?;

            let secrets_path = dir_entry.path().join("Secrets.toml");

            if dir_entry.file_type().context("get file type")?.is_dir() {
                // Make sure to add any `Secrets.toml` files.
                if secrets_path.exists() {
                    let path = secrets_path
                        .strip_prefix(base_directory)
                        .context("strip the base of the archive entry")?
                        .to_path_buf();
                    entries.insert(secrets_path.clone(), path.clone());
                }

                // It's not possible to add a directory to an archive
                continue;
            }

            let path = dir_entry
                .path()
                .strip_prefix(base_directory)
                .context("strip the base of the archive entry")?;

            entries.insert(dir_entry.path().to_path_buf(), path.to_path_buf());
        }

        let secrets_path = self.ctx.working_directory().join("Secrets.toml");
        if secrets_path.exists() {
            entries.insert(secrets_path, Path::new("shuttle").join("Secrets.toml"));
        }

        // Append all the entries to the archive.
        for (k, v) in entries {
            debug!("Packing {k:?}");
            tar.append_path_with_name(k, v)?;
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

            let repo_path = dunce::canonicalize(repo_path)?;

            trace!(?repo_path, "found git repository");

            let repo_rel_path = working_directory
                .strip_prefix(repo_path.as_path())
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
                writeln!(error, "To proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag").expect("to append error");

                bail!(error);
            }
        }

        Ok(())
    }
}

fn check_version(runtime_path: &Path) -> Result<()> {
    let valid_version = semver::Version::from_str(VERSION)
        .context("failed to convert runtime version to semver")?
        .to_string();

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
    .context("failed to convert runtime version to semver")?
    .to_string();

    if runtime_version == valid_version {
        Ok(())
    } else {
        bail!("shuttle-runtime and cargo-shuttle are not the same version")
    }
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

    pub fn path_from_workspace_root(path: &str) -> PathBuf {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join(path);

        dunce::canonicalize(path).unwrap()
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
    fn load_project_returns_proper_working_directory_in_project_args() {
        let mut project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
        };

        let mut shuttle = Shuttle::new().unwrap();
        Shuttle::load_project(&mut shuttle, &mut project_args).unwrap();

        assert_eq!(
            project_args.working_directory,
            path_from_workspace_root("examples/axum/hello-world/src")
        );
        assert_eq!(
            project_args.workspace_path().unwrap(),
            path_from_workspace_root("examples/axum/hello-world")
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
                "src/main.rs",
            ]
        );
    }

    #[test]
    fn make_archive_respect_ignore() {
        let tmp_dir = TempDir::new().unwrap();
        let working_directory = tmp_dir.path();

        fs::write(working_directory.join(".env"), "API_KEY = 'blabla'").unwrap();
        fs::write(working_directory.join(".ignore"), ".env").unwrap();
        fs::write(
            working_directory.join("Cargo.toml"),
            r#"
[package]
name = "secrets"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::create_dir_all(working_directory.join("src")).unwrap();
        fs::write(
            working_directory.join("src").join("main.rs"),
            "fn main() {}",
        )
        .unwrap();

        let project_args = ProjectArgs {
            working_directory: working_directory.to_path_buf(),
            name: Some(ProjectName::from_str("secret").unwrap()),
        };

        let mut entries = get_archive_entries(project_args);
        entries.sort();

        assert_eq!(
            entries,
            vec![".ignore", "Cargo.lock", "Cargo.toml", "src/main.rs"]
        );
    }

    #[test]
    fn make_archive_ignore_target_folder() {
        let tmp_dir = TempDir::new().unwrap();
        let working_directory = tmp_dir.path();

        fs::create_dir_all(working_directory.join("target")).unwrap();
        fs::write(working_directory.join("target").join("binary"), "12345").unwrap();
        fs::write(
            working_directory.join("Cargo.toml"),
            r#"
[package]
name = "exclude_target"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::create_dir_all(working_directory.join("src")).unwrap();
        fs::write(
            working_directory.join("src").join("main.rs"),
            "fn main() {}",
        )
        .unwrap();

        let project_args = ProjectArgs {
            working_directory: working_directory.to_path_buf(),
            name: Some(ProjectName::from_str("exclude_target").unwrap()),
        };

        let mut entries = get_archive_entries(project_args);
        entries.sort();

        assert_eq!(entries, vec!["Cargo.lock", "Cargo.toml", "src/main.rs"]);
    }
}
