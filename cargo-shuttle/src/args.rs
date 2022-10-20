use std::{
    ffi::OsStr,
    fs::{canonicalize, create_dir_all},
    io::{self, ErrorKind},
    path::PathBuf,
};

use clap::Parser;
use clap_complete::Shell;
use shuttle_common::project::ProjectName;
use uuid::Uuid;

#[derive(Parser)]
#[clap(
    version,
    about,
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(clap::Arg::with_name("dummy")
        .possible_value("shuttle")
        .required(false)
        .hidden(true))
)]
pub struct Args {
    /// run this command against the api at the supplied url
    /// (allows targeting a custom deployed instance for this command only)
    #[clap(long, env = "SHUTTLE_API")]
    pub api_url: Option<String>,
    #[clap(flatten)]
    pub project_args: ProjectArgs,
    #[clap(subcommand)]
    pub cmd: Command,
}

// Common args for subcommands that deal with projects.
#[derive(Parser, Debug)]
pub struct ProjectArgs {
    /// Specify the working directory
    #[clap(
        global = true,
        long,
        parse(try_from_os_str = parse_path),
        default_value = ".",
    )]
    pub working_directory: PathBuf,
    /// Specify the name of the project (overrides crate name)
    #[clap(global = true, long)]
    pub name: Option<ProjectName>,
}

#[derive(Parser)]
pub enum Command {
    /// deploy a shuttle service
    Deploy(DeployArgs),
    /// manage deployments of a shuttle service
    #[clap(subcommand)]
    Deployment(DeploymentCommand),
    /// create a new shuttle service
    Init(InitArgs),
    /// generate shell completions
    Generate {
        /// which shell
        #[clap(short, long, env, default_value_t = Shell::Bash)]
        shell: Shell,
        /// output to file or stdout by default
        #[clap(short, long, env)]
        output: Option<PathBuf>,
    },
    /// view the status of a shuttle service
    Status,
    /// view the logs of a deployment in this shuttle service
    Logs {
        /// Deployment ID to get logs for. Defaults to currently running deployment
        id: Option<Uuid>,

        #[clap(short, long)]
        /// Follow log output
        follow: bool,
    },
    /// delete this shuttle service
    Delete,
    /// manage secrets for this shuttle service
    Secrets,
    /// create user credentials for the shuttle platform
    Auth(AuthArgs),
    /// login to the shuttle platform
    Login(LoginArgs),
    /// run a shuttle service locally
    Run(RunArgs),
    /// manage a project on shuttle
    #[clap(subcommand)]
    Project(ProjectCommand),
}

#[derive(Parser)]
pub enum DeploymentCommand {
    /// list all the deployments for a service
    List,
    /// view status of a deployment
    Status {
        /// ID of deployment to get status for
        id: Uuid,
    },
}

#[derive(Parser)]
pub enum ProjectCommand {
    /// create an environment for this project on shuttle
    New,
    /// remove this project environment from shuttle
    Rm,
    /// show the status of this project's environment on shuttle
    Status,
}

#[derive(Parser)]
pub struct LoginArgs {
    /// api key for the shuttle platform
    #[clap(long)]
    pub api_key: Option<String>,
}

#[derive(Parser)]
pub struct AuthArgs {
    /// the desired username for the shuttle platform
    #[clap()]
    pub username: String,
}

#[derive(Parser)]
pub struct DeployArgs {
    /// allow dirty working directories to be packaged
    #[clap(long)]
    pub allow_dirty: bool,
    /// allows pre-deploy tests to be skipped
    #[clap(long)]
    pub no_test: bool,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// port to start service on
    #[clap(long, default_value = "8000")]
    pub port: u16,
}

#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Initialize with axum framework
    #[clap(long, conflicts_with_all = &["rocket", "tide", "tower", "poem", "serenity", "warp", "salvo", "thruster"])]
    pub axum: bool,
    /// Initialize with rocket framework
    #[clap(long, conflicts_with_all = &["axum", "tide", "tower", "poem", "serenity", "warp", "salvo", "thruster"])]
    pub rocket: bool,
    /// Initialize with tide framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tower", "poem", "serenity", "warp", "salvo", "thruster"])]
    pub tide: bool,
    /// Initialize with tower framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tide", "poem", "serenity", "warp", "salvo", "thruster"])]
    pub tower: bool,
    /// Initialize with poem framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tide", "tower", "serenity", "warp", "salvo", "thruster"])]
    pub poem: bool,
    /// Initialize with salvo framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tide", "tower", "poem", "warp", "serenity", "thruster"])]
    pub salvo: bool,
    /// Initialize with serenity framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tide", "tower", "poem", "warp", "salvo", "thruster"])]
    pub serenity: bool,
    /// Initialize with warp framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tide", "tower", "poem", "serenity", "salvo", "thruster"])]
    pub warp: bool,
    /// Initialize with thruster framework
    #[clap(long, conflicts_with_all = &["axum", "rocket", "tide", "tower", "poem", "warp", "salvo", "serenity"])]
    pub thruster: bool,
    /// Path to initialize a new shuttle project
    #[clap(
        parse(try_from_os_str = parse_init_path),
        default_value = ".",
    )]
    pub path: PathBuf,
}

// Helper function to parse and return the absolute path
fn parse_path(path: &OsStr) -> Result<PathBuf, io::Error> {
    canonicalize(path).map_err(|e| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("could not turn {path:?} into a real path: {e}"),
        )
    })
}

// Helper function to parse, create if not exists, and return the absolute path
fn parse_init_path(path: &OsStr) -> Result<PathBuf, io::Error> {
    // Create the directory if does not exist
    create_dir_all(path)?;

    parse_path(path)
}
