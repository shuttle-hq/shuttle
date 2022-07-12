use std::{
    ffi::OsStr,
    fs::{canonicalize, create_dir_all},
    io::{self, ErrorKind},
    path::PathBuf,
};

use clap::Parser;
use shuttle_common::project::ProjectName;

#[derive(Parser)]
#[clap(
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(clap::Arg::with_name("dummy")
        .possible_value("shuttle")
        .required(false)
        .hidden(true))
)]
pub struct Args {
    #[clap(
        long,
        help = "allows targeting a custom deloyed instance for this command only",
        env = "SHUTTLE_API"
    )]
    /// Run this command against the api at the supplied url
    pub api_url: Option<String>,
    #[clap(flatten)]
    pub project_args: ProjectArgs,
    #[clap(subcommand)]
    pub cmd: Command,
}

// Common args for subcommands that deal with projects.
#[derive(Parser, Debug)]
pub struct ProjectArgs {
    #[clap(
        global = true,
        long,
        parse(try_from_os_str = parse_path),
        default_value = ".",
    )]
    /// Specify the working directory
    pub working_directory: PathBuf,
    #[clap(global = true, long)]
    /// Specify the name of the project (overrides crate name)
    pub name: Option<ProjectName>,
}

#[derive(Parser)]
pub enum Command {
    #[clap(help = "deploy a shuttle project")]
    Deploy(DeployArgs),
    #[clap(help = "create a new shuttle project")]
    Init(InitArgs),
    #[clap(help = "view the status of a shuttle project")]
    Status,
    #[clap(help = "view the logs of a shuttle project")]
    Logs,
    #[clap(help = "delete the latest deployment for a shuttle project")]
    Delete,
    #[clap(help = "create user credentials for the shuttle platform")]
    Auth(AuthArgs),
    #[clap(help = "login to the shuttle platform")]
    Login(LoginArgs),
    #[clap(help = "run a shuttle project locally")]
    Run(RunArgs),
}

#[derive(Parser)]
pub struct LoginArgs {
    #[clap(long, help = "api key for the shuttle platform")]
    pub api_key: Option<String>,
}

#[derive(Parser)]
pub struct AuthArgs {
    #[clap(help = "the desired username for the shuttle platform")]
    pub username: String,
}

#[derive(Parser)]
pub struct DeployArgs {
    #[clap(long, help = "allow dirty working directories to be packaged")]
    pub allow_dirty: bool,
    #[clap(long, help = "allows pre-deploy tests to be skipped")]
    pub no_test: bool,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    #[clap(long, help = "port to start service on", default_value = "8000")]
    pub port: u16,
}

#[derive(Parser)]
pub struct InitArgs {
    #[clap(
        help = "the path to initialize a new shuttle project",
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
